use vu_meter_service::capture::ChannelLevels;
use vu_meter_service::{decibel, protocol};

const MIN_DB: f64 = -60.0;
const MAX_DB: f64 = 0.0;
const REF_S32: f64 = 2_147_483_648.0;

fn channel_from_samples(samples: &[i32]) -> ChannelLevels {
    let rms_db = decibel::calculate_rms_db(samples, REF_S32, MIN_DB, MAX_DB);
    let peak_db = decibel::calculate_peak_db(samples, REF_S32, MIN_DB, MAX_DB);

    ChannelLevels {
        rms_u8: decibel::db_to_u8(rms_db, MIN_DB, MAX_DB),
        peak_u8: decibel::db_to_u8(peak_db, MIN_DB, MAX_DB),
        clipping: decibel::detect_clipping(samples, REF_S32),
    }
}

#[test]
fn silent_channels_encode_as_zero_frame() {
    let channels = [channel_from_samples(&[]), channel_from_samples(&[])];
    let frame = protocol::build_levels_frame(&channels);

    assert_eq!(frame[0], 0);
    assert_eq!(frame[1], 0);
    assert_eq!(frame[2], 0);
    assert_eq!(frame[3], 0);
    assert_eq!(frame[4], 0);
    assert_eq!(frame[5], 2);
}

#[test]
fn hot_signal_sets_clip_flag_and_nonzero_levels() {
    let left = channel_from_samples(&[i32::MAX, i32::MAX]);
    let right = channel_from_samples(&[1_000, 2_000, 1_500]);
    let frame = protocol::build_levels_frame(&[left, right]);

    assert!(frame[0] > 0);
    assert!(frame[1] > 0);
    assert!(frame[4] & 0x01 == 0x01);
    assert_eq!(frame[5], 2);
}

#[test]
fn min_i32_sample_path_does_not_panic() {
    let result = std::panic::catch_unwind(|| {
        let ch = channel_from_samples(&[i32::MIN]);
        protocol::build_levels_frame(&[ch]);
    });

    assert!(result.is_ok());
}
