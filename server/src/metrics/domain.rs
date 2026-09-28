//! Sample windows and buckets for the bucketed metrics reads.

use chrono::{DateTime, Duration, Utc};

/// At most this many points per series.
pub const MAX_POINTS: i64 = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleRange {
    window_seconds: i64,
    bucket_seconds: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SampleRangeError {
    #[error("window must be one of 5m, 15m, 1h, 24h, 7d")]
    Window,
    #[error("bucket must be a duration like 5s, 60s, 15m, or 1h")]
    Bucket,
    #[error("bucket must divide the window into at most 200 points")]
    Points,
}

impl SampleRange {
    pub fn parse(window: &str, bucket: &str) -> Result<Self, SampleRangeError> {
        let window_seconds = match window {
            "5m" => 300,
            "15m" => 900,
            "1h" => 3_600,
            "24h" => 86_400,
            "7d" => 604_800,
            _ => return Err(SampleRangeError::Window),
        };
        let bucket_seconds = parse_duration(bucket).ok_or(SampleRangeError::Bucket)?;
        if window_seconds % bucket_seconds != 0 || window_seconds / bucket_seconds > MAX_POINTS {
            return Err(SampleRangeError::Points);
        }
        Ok(Self {
            window_seconds,
            bucket_seconds,
        })
    }

    #[must_use]
    pub const fn bucket_seconds(self) -> i64 {
        self.bucket_seconds
    }

    #[must_use]
    pub const fn points(self) -> i64 {
        self.window_seconds / self.bucket_seconds
    }

    /// Bucket starts ending with the bucket that contains `now`, aligned to the Unix epoch.
    #[must_use]
    pub fn bucket_starts(self, now: DateTime<Utc>) -> Vec<DateTime<Utc>> {
        let current = now.timestamp().div_euclid(self.bucket_seconds) * self.bucket_seconds;
        let first = current - (self.points() - 1) * self.bucket_seconds;
        (0..self.points())
            .filter_map(|index| DateTime::from_timestamp(first + index * self.bucket_seconds, 0))
            .collect()
    }

    /// `[start, end)` covering every bucket.
    #[must_use]
    pub fn bounds(self, now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
        let starts = self.bucket_starts(now);
        let start = starts.first().copied().unwrap_or(now);
        let end = starts
            .last()
            .map_or(now, |last| *last + Duration::seconds(self.bucket_seconds));
        (start, end)
    }
}

/// `5s`, `60s`, `15m`, `1h`: a positive whole number and one unit.
fn parse_duration(value: &str) -> Option<i64> {
    let split = value.find(|character: char| !character.is_ascii_digit())?;
    let (digits, unit) = value.split_at(split);
    if digits.is_empty() || digits.len() > 6 || digits.starts_with('0') {
        return None;
    }
    let amount: i64 = digits.parse().ok()?;
    let unit_seconds = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        _ => return None,
    };
    Some(amount * unit_seconds)
}

/// Whether the series covers one node or the whole cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SampleScope {
    Node(String),
    Cluster,
}

/// One bucket. Figures are `None` when no sample landed in it, never 0. For the cluster, CPU is
/// the mean over contributing nodes (0–100 of the cluster) and the other figures are sums.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplePoint {
    pub at: DateTime<Utc>,
    pub cpu_percent: Option<f64>,
    pub memory_used_bytes: Option<f64>,
    pub disk_used_bytes: Option<f64>,
    pub containers_running: Option<f64>,
    pub net_rx_bytes_per_sec: Option<f64>,
    pub net_tx_bytes_per_sec: Option<f64>,
    /// Nodes with at least one sample in the bucket.
    pub nodes: u32,
}

impl SamplePoint {
    #[must_use]
    pub const fn empty(at: DateTime<Utc>) -> Self {
        Self {
            at,
            cpu_percent: None,
            memory_used_bytes: None,
            disk_used_bytes: None,
            containers_running: None,
            net_rx_bytes_per_sec: None,
            net_tx_bytes_per_sec: None,
            nodes: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{SampleRange, SampleRangeError};

    #[test]
    fn windows_and_buckets_are_closed_and_bounded() {
        assert_eq!(SampleRange::parse("24h", "15m").unwrap().points(), 96);
        assert_eq!(SampleRange::parse("1h", "60s").unwrap().points(), 60);
        assert_eq!(SampleRange::parse("5m", "5s").unwrap().points(), 60);
        assert_eq!(SampleRange::parse("7d", "1h").unwrap().points(), 168);
        assert_eq!(SampleRange::parse("15m", "15m").unwrap().points(), 1);

        assert_eq!(
            SampleRange::parse("2h", "1m"),
            Err(SampleRangeError::Window)
        );
        for bucket in [
            "", "s", "0s", "05s", "5", "5d", "1.5m", "-5s", "5 s", "9999999s",
        ] {
            assert_eq!(
                SampleRange::parse("1h", bucket),
                Err(SampleRangeError::Bucket),
                "{bucket}"
            );
        }
        assert_eq!(
            SampleRange::parse("1h", "7s"),
            Err(SampleRangeError::Points),
            "does not divide"
        );
        assert_eq!(
            SampleRange::parse("24h", "60s"),
            Err(SampleRangeError::Points),
            "1440 points"
        );
        assert_eq!(
            SampleRange::parse("5m", "10m"),
            Err(SampleRangeError::Points)
        );
    }

    #[test]
    fn buckets_align_to_the_epoch_and_end_with_the_current_one() {
        let range = SampleRange::parse("5m", "60s").unwrap();
        let now = Utc.with_ymd_and_hms(2026, 9, 28, 12, 3, 42).unwrap();
        let starts = range.bucket_starts(now);
        assert_eq!(starts.len(), 5);
        assert_eq!(
            starts[0],
            Utc.with_ymd_and_hms(2026, 9, 28, 11, 59, 0).unwrap()
        );
        assert_eq!(
            starts[4],
            Utc.with_ymd_and_hms(2026, 9, 28, 12, 3, 0).unwrap()
        );
        let (start, end) = range.bounds(now);
        assert_eq!(start, starts[0]);
        assert_eq!(end, Utc.with_ymd_and_hms(2026, 9, 28, 12, 4, 0).unwrap());
    }
}
