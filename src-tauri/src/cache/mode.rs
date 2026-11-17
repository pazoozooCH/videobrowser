//! Frame modes, and the timestamps of a mode for a duration. Frames are kept by mode key and
//! index, so the timestamps are worked out here alone.

use serde::{Deserialize, Serialize};

const MAX_FRAMES: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum FrameMode {
    /// `count` frames spread evenly, leaving out the very start and end.
    Fixed { count: u32 },
    /// One frame every `minutes`, from the first interval on.
    Interval { minutes: f64 },
}

impl FrameMode {
    /// The key frames are stored under: `fixed:9`, `interval:60` (seconds).
    pub fn key(&self) -> String {
        match self {
            FrameMode::Fixed { count } => format!("fixed:{}", count),
            FrameMode::Interval { minutes } => format!("interval:{}", (minutes * 60.0).round() as u64),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            FrameMode::Fixed { count } if *count as usize > MAX_FRAMES => {
                Err(format!("At most {} frames", MAX_FRAMES))
            }
            FrameMode::Interval { minutes } if !(minutes.is_finite() && (minutes * 60.0).round() >= 1.0) => {
                Err("Interval must be at least one second".to_string())
            }
            _ => Ok(()),
        }
    }

    pub fn timestamps(&self, duration_secs: f64) -> Vec<f64> {
        if !(duration_secs.is_finite() && duration_secs > 0.0) {
            return Vec::new();
        }
        match *self {
            FrameMode::Fixed { count } => {
                let step = duration_secs / (count as f64 + 1.0);
                (1..=count).map(|i| step * i as f64).collect()
            }
            FrameMode::Interval { .. } => {
                let interval = self.interval_secs();
                (1..)
                    .map(|i| interval * i as f64)
                    .take_while(|t| *t < duration_secs)
                    .take(MAX_FRAMES)
                    .collect()
            }
        }
    }

    fn interval_secs(&self) -> f64 {
        match self {
            FrameMode::Interval { minutes } => (minutes * 60.0).round().max(1.0),
            FrameMode::Fixed { .. } => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_mode_timestamps() {
        let ts = FrameMode::Fixed { count: 9 }.timestamps(100.0);
        assert_eq!(ts.len(), 9);
        assert!((ts[0] - 10.0).abs() < 0.01);
        assert!((ts[4] - 50.0).abs() < 0.01);
        assert!((ts[8] - 90.0).abs() < 0.01);
    }

    #[test]
    fn test_fixed_mode_zero_count() {
        assert!(FrameMode::Fixed { count: 0 }.timestamps(100.0).is_empty());
    }

    #[test]
    fn test_interval_mode_timestamps() {
        // 10 minute video, frame every 2 minutes => at 120, 240, 360, 480
        let ts = FrameMode::Interval { minutes: 2.0 }.timestamps(600.0);
        assert_eq!(ts, vec![120.0, 240.0, 360.0, 480.0]);
    }

    #[test]
    fn test_interval_mode_short_video() {
        assert!(FrameMode::Interval { minutes: 1.0 }.timestamps(30.0).is_empty());
    }

    #[test]
    fn test_no_duration_gives_no_frames() {
        assert!(FrameMode::Fixed { count: 9 }.timestamps(0.0).is_empty());
        assert!(FrameMode::Fixed { count: 9 }.timestamps(f64::NAN).is_empty());
    }

    #[test]
    fn test_keys() {
        assert_eq!(FrameMode::Fixed { count: 18 }.key(), "fixed:18");
        assert_eq!(FrameMode::Interval { minutes: 5.0 }.key(), "interval:300");
        assert_eq!(FrameMode::Interval { minutes: 0.5 }.key(), "interval:30");
    }

    #[test]
    fn test_validate() {
        assert!(FrameMode::Interval { minutes: 0.0 }.validate().is_err());
        assert!(FrameMode::Interval { minutes: f64::INFINITY }.validate().is_err());
        assert!(FrameMode::Fixed { count: 100_000 }.validate().is_err());
        assert!(FrameMode::Fixed { count: 9 }.validate().is_ok());
    }

    #[test]
    fn test_deserializes_frontend_shape() {
        let fixed: FrameMode = serde_json::from_str(r#"{"type":"fixed","count":9}"#).unwrap();
        assert_eq!(fixed, FrameMode::Fixed { count: 9 });
        let interval: FrameMode = serde_json::from_str(r#"{"type":"interval","minutes":1}"#).unwrap();
        assert_eq!(interval, FrameMode::Interval { minutes: 1.0 });
    }
}
