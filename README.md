# HiFiBerry VU Meter Service

Real-time audio level monitoring for HiFiBerry OS. Captures audio from PipeWire and streams levels to the web UI via WebSocket.

## How it works

The service connects to the PipeWire audio graph, captures PCM samples from the default audio sink's monitor, and computes RMS/peak levels for each channel. Levels are streamed to connected WebSocket clients as compact 6-byte binary frames at 10 Hz.

## Rust code analysis

### Module overview

- `src/main.rs` hosts the Axum HTTP/WebSocket server, version endpoints, and shutdown handling.
- `src/capture.rs` owns PipeWire integration, background sample capture, buffering, and periodic level computation.
- `src/decibel.rs` contains pure signal math: RMS/peak, dB conversion, clipping detection, and u8 scaling.
- `src/protocol.rs` encodes computed levels into the fixed 6-byte wire format.
- `src/lib.rs` re-exports modules for use by integration tests and the binary.

### Runtime flow

1. `main()` reads `VU_METER_PORT` and optional `VU_METER_TARGET`.
2. `capture::start_capture()` initializes shared state and launches:
     - a PipeWire thread (`run_pipewire_loop`) that fills per-channel `Vec<i32>` buffers,
     - a processing thread that wakes every 100 ms and computes channel levels.
3. WebSocket clients connect to `/api/v1/levels`; each connection sends one frame every 100 ms from shared meter state.
4. On disconnect or `Ctrl+C`, quit flags and client counters are updated.

### Concurrency and backpressure behavior

- Shared data is protected with `Arc<Mutex<...>>`:
    - sample buffers between PipeWire callback and processing thread,
    - latest `MeterState` between processing thread and WebSocket tasks.
- Active clients are tracked via `Arc<AtomicUsize>`.
- If no clients are connected, processing intentionally drains buffers and resets levels to zero. This avoids unbounded buffer growth and unnecessary CPU work while idle.
- Processing uses fixed windows (`frames_per_update` at 48 kHz, 100 ms) to produce stable 10 Hz meter updates.

### Signal processing details

- Input format is `S32LE`, 2 channels, 48 kHz.
- RMS and peak are computed per channel, then converted to dB with clamping to `[-60 dB, 0 dB]`.
- dB values are linearly scaled into `u8` (`0..255`) for compact transport.
- Clipping is flagged when absolute sample magnitude reaches at least `0.999 * reference`.
- `unsigned_abs()` is used to safely handle `i32::MIN` without overflow.

### Protocol design notes

- The 6-byte frame is fixed-size and allocation-light on the encode path.
- Byte 4 carries clipping flags (`0x01` left, `0x02` right).
- Byte 5 carries the channel count, so clients can detect mono/stereo scenarios.
- Silence naturally maps to all-zero levels/flags.

### Testing coverage

- `src/decibel.rs` unit tests validate edge behavior (empty input, clamping, clipping threshold, `i32::MIN`).
- `src/protocol.rs` unit tests validate frame packing and flag bits.
- `tests/levels_protocol_integration.rs` checks end-to-end conversion from samples to encoded frames and panic safety on extreme values.

### Potential future improvements

- Replace the `Mutex<Vec<Vec<i32>>>` sample buffer with a lock-free ring buffer to reduce contention under heavy callback pressure.
- Add monotonic timestamps or sequence numbers in the frame if clients need jitter/loss diagnostics.
- Add configurable update rate and dB floor via environment variables for deployment tuning.

## WebSocket API

**Endpoint:** `ws://localhost:2717/api/v1/levels`

Each frame is 6 bytes:

| Byte | Description |
|------|-------------|
| 0 | Left channel RMS (0–255, maps -60 dB to 0 dB) |
| 1 | Left channel peak (0–255) |
| 2 | Right channel RMS (0–255) |
| 3 | Right channel peak (0–255) |
| 4 | Flags (bit 0: left clipping, bit 1: right clipping) |
| 5 | Number of channels |

Silence produces all zeros. No subscription message needed — connect and start receiving.

## REST API

- `GET /api/v1/version` — returns `{ "version": "...", "api_version": "1.0" }`
- `GET /version` — same as above

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `VU_METER_PORT` | `2717` | HTTP/WebSocket listen port |
| `VU_METER_TARGET` | *(auto)* | PipeWire node name to monitor (auto-detects default sink if unset) |

## Building

```bash
cargo build --release
```

Requires `libpipewire-0.3-dev` and `libclang-dev`.

## Debian package

```bash
# From the packages/vu-meter/ directory in hifiberry-os:
./build.sh
```

Installs:
- `/usr/bin/vu-meter-service` — the binary
- `/usr/lib/systemd/user/vu-meter.service` — systemd user service (runs after PipeWire)
- Nginx proxy config for `/api/vu-meter/` → `localhost:2717`

## Running

The service runs as a systemd user service alongside PipeWire:

```bash
systemctl --user enable vu-meter
systemctl --user start vu-meter
```

## Testing

Use the included script `vu-meter-test.py`:

```bash
python3 vu-meter-test.py <ip>
```

Examples:

```bash
python3 vu-meter-test.py 192.168.8.104
python3 vu-meter-test.py 192.168.8.104 --port 2717
python3 vu-meter-test.py 192.168.8.104 --reconnect-delay 0.5
```

Script behavior:

- Connects to `ws://<ip>:<port>/api/v1/levels`
- Prints parsed 6-byte level frames continuously
- Automatically reconnects after disconnects/timeouts

If needed, install the client dependency:

```bash
python3 -m pip install websockets
```

Current script implementation:

```python
import argparse
import asyncio
import sys

import websockets
from websockets.exceptions import ConnectionClosedError

async def main(ip_address: str, port: int, reconnect_delay: float):
    uri = f"ws://{ip_address}:{port}/api/v1/levels"

    while True:
        try:
            async with websockets.connect(uri, ping_interval=20, ping_timeout=None) as ws:
                async for msg in ws:
                    data = list(msg)
                    if len(data) < 6:
                        continue
                    print(
                        f"L: rms={data[0]} peak={data[1]}  R: rms={data[2]} peak={data[3]}  "
                        f"flags={data[4]:02b}  ch={data[5]}"
                    )
        except (ConnectionClosedError, TimeoutError, OSError) as exc:
            print(
                f"Connection lost ({exc}); reconnecting in {reconnect_delay:.1f}s...",
                file=sys.stderr,
            )
            await asyncio.sleep(reconnect_delay)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Listen to VU meter level frames over WebSocket")
    parser.add_argument("ip", help="IP address of the VU meter service host")
    parser.add_argument("--port", type=int, default=2717, help="WebSocket port (default: 2717)")
    parser.add_argument(
        "--reconnect-delay",
        type=float,
        default=1.0,
        help="Reconnect delay in seconds after disconnect (default: 1.0)",
    )
    args = parser.parse_args()

    asyncio.run(main(args.ip, args.port, args.reconnect_delay))
```

## License

MIT
