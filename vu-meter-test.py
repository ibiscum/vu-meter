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