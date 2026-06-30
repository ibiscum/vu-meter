use crate::capture::ChannelLevels;

/// Build the fixed 6-byte binary frame used by the WebSocket API:
/// [L_rms, L_peak, R_rms, R_peak, flags, num_channels].
pub fn build_levels_frame(channels: &[ChannelLevels]) -> [u8; 6] {
    let num_channels = channels.len();
    let mut buf = [0u8; 6];

    if num_channels >= 1 {
        buf[0] = channels[0].rms_u8;
        buf[1] = channels[0].peak_u8;
    }
    if num_channels >= 2 {
        buf[2] = channels[1].rms_u8;
        buf[3] = channels[1].peak_u8;
    }

    let mut flags: u8 = 0;
    if num_channels >= 1 && channels[0].clipping {
        flags |= 0x01;
    }
    if num_channels >= 2 && channels[1].clipping {
        flags |= 0x02;
    }

    buf[4] = flags;
    buf[5] = num_channels.min(u8::MAX as usize) as u8;
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_is_zeroed_for_no_channels() {
        let frame = build_levels_frame(&[]);
        assert_eq!(frame, [0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn frame_encodes_one_channel_and_flags() {
        let channels = [ChannelLevels {
            rms_u8: 12,
            peak_u8: 34,
            clipping: true,
        }];

        let frame = build_levels_frame(&channels);
        assert_eq!(frame, [12, 34, 0, 0, 0x01, 1]);
    }

    #[test]
    fn frame_encodes_two_channels_and_both_clips() {
        let channels = [
            ChannelLevels {
                rms_u8: 1,
                peak_u8: 2,
                clipping: true,
            },
            ChannelLevels {
                rms_u8: 3,
                peak_u8: 4,
                clipping: true,
            },
        ];

        let frame = build_levels_frame(&channels);
        assert_eq!(frame, [1, 2, 3, 4, 0x03, 2]);
    }
}
