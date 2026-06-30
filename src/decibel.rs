/// Calculate RMS (Root Mean Square) value from audio samples
pub fn calculate_rms(samples: &[i32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_squares: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    (sum_squares / samples.len() as f64).sqrt()
}

/// Calculate peak (maximum absolute) value from audio samples
pub fn calculate_peak(samples: &[i32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples
        .iter()
        .map(|&s| s.unsigned_abs() as f64)
        .fold(0.0, f64::max)
}

/// Convert RMS value to decibels relative to a reference value
pub fn rms_to_db(rms: f64, reference: f64, min_db: f64) -> f64 {
    if rms < 1.0 {
        return min_db;
    }
    let db = 20.0 * (rms / reference).log10();
    db.max(min_db)
}

/// Calculate RMS in decibels from audio samples
pub fn calculate_rms_db(samples: &[i32], reference: f64, min_db: f64, max_db: f64) -> f64 {
    let rms = calculate_rms(samples);
    rms_to_db(rms, reference, min_db).max(min_db).min(max_db)
}

/// Calculate peak in decibels from audio samples
pub fn calculate_peak_db(samples: &[i32], reference: f64, min_db: f64, max_db: f64) -> f64 {
    let peak = calculate_peak(samples);
    if peak < 1.0 {
        return min_db;
    }
    let db = 20.0 * (peak / reference).log10();
    db.max(min_db).min(max_db)
}

/// Detect if any samples exceed a clipping threshold
pub fn detect_clipping(samples: &[i32], reference: f64) -> bool {
    let threshold = (reference * 0.999).max(0.0);
    samples
        .iter()
        .any(|&s| (s.unsigned_abs() as f64) >= threshold)
}

/// Map a dB value to a u8 (0-255) for the binary WebSocket protocol.
/// Maps min_db → 0, max_db → 255.
pub fn db_to_u8(db: f64, min_db: f64, max_db: f64) -> u8 {
    let range = max_db - min_db;
    if range <= 0.0 {
        return 0;
    }
    let normalized = ((db - min_db) / range).clamp(0.0, 1.0);
    (normalized * 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_is_zero_for_empty_input() {
        assert_eq!(calculate_rms(&[]), 0.0);
    }

    #[test]
    fn peak_handles_i32_min_without_overflow() {
        let peak = calculate_peak(&[i32::MIN, -1, 1]);
        assert_eq!(peak, 2_147_483_648.0);
    }

    #[test]
    fn rms_to_db_returns_floor_for_tiny_signal() {
        assert_eq!(rms_to_db(0.5, 1.0, -60.0), -60.0);
    }

    #[test]
    fn rms_db_is_clamped_to_max() {
        let db = calculate_rms_db(&[i32::MAX], 1.0, -60.0, 0.0);
        assert_eq!(db, 0.0);
    }

    #[test]
    fn peak_db_is_min_for_empty_input() {
        let db = calculate_peak_db(&[], 2_147_483_648.0, -60.0, 0.0);
        assert_eq!(db, -60.0);
    }

    #[test]
    fn clipping_threshold_detects_edge_and_i32_min() {
        assert!(!detect_clipping(&[998], 1000.0));
        assert!(detect_clipping(&[999], 1000.0));
        assert!(detect_clipping(&[i32::MIN], 2_147_483_648.0));
    }

    #[test]
    fn db_to_u8_clamps_and_handles_invalid_range() {
        assert_eq!(db_to_u8(-60.0, -60.0, 0.0), 0);
        assert_eq!(db_to_u8(0.0, -60.0, 0.0), 255);
        assert_eq!(db_to_u8(12.0, -60.0, 0.0), 255);
        assert_eq!(db_to_u8(-120.0, -60.0, 0.0), 0);
        assert_eq!(db_to_u8(-10.0, 0.0, 0.0), 0);
    }
}
