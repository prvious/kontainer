#![forbid(unsafe_code)]

use std::{
    fmt,
    str::FromStr,
    sync::OnceLock,
    time::{Duration, Instant},
};

pub const DEFAULT_SHUTDOWN_GRACE: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShutdownGrace(Duration);

impl ShutdownGrace {
    pub fn duration(self) -> Duration {
        self.0
    }
}

impl Default for ShutdownGrace {
    fn default() -> Self {
        Self(DEFAULT_SHUTDOWN_GRACE)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidShutdownGrace;

impl fmt::Display for InvalidShutdownGrace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .write_str("shutdown_grace must be a positive, representable duration (ms, s, or m)")
    }
}

impl std::error::Error for InvalidShutdownGrace {}

impl FromStr for ShutdownGrace {
    type Err = InvalidShutdownGrace;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (number, unit) = value
            .strip_suffix("ms")
            .map(|number| (number, "ms"))
            .or_else(|| value.strip_suffix('s').map(|number| (number, "s")))
            .or_else(|| value.strip_suffix('m').map(|number| (number, "m")))
            .ok_or(InvalidShutdownGrace)?;
        if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(InvalidShutdownGrace);
        }
        let amount = number.parse::<u64>().map_err(|_| InvalidShutdownGrace)?;
        if amount == 0 {
            return Err(InvalidShutdownGrace);
        }
        let duration = match unit {
            "ms" => Duration::from_millis(amount),
            "s" => Duration::from_secs(amount),
            "m" => Duration::from_secs(amount.checked_mul(60).ok_or(InvalidShutdownGrace)?),
            _ => unreachable!(),
        };
        Instant::now()
            .checked_add(duration)
            .ok_or(InvalidShutdownGrace)?;
        Ok(Self(duration))
    }
}

#[derive(Debug)]
pub struct DrainClock {
    grace: ShutdownGrace,
    deadline: OnceLock<Instant>,
}

impl DrainClock {
    pub fn new(grace: ShutdownGrace) -> Self {
        Self {
            grace,
            deadline: OnceLock::new(),
        }
    }

    /// The first caller fixes the absolute deadline; later causes cannot extend it.
    pub fn begin(&self) -> Instant {
        *self.deadline.get_or_init(|| {
            Instant::now()
                .checked_add(self.grace.duration())
                .expect("validated shutdown grace remains representable")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_is_positive_representable_and_has_one_default() {
        assert_eq!(ShutdownGrace::default().duration(), Duration::from_secs(20));
        assert_eq!(
            "250ms".parse::<ShutdownGrace>().unwrap().duration(),
            Duration::from_millis(250)
        );
        assert_eq!(
            "2m".parse::<ShutdownGrace>().unwrap().duration(),
            Duration::from_secs(120)
        );
        for invalid in [
            "",
            "0s",
            "0ms",
            "-1s",
            "+1s",
            "1.5s",
            " 1s",
            "1s ",
            "1h",
            "18446744073709551616s",
            "18446744073709551615m",
        ] {
            assert!(invalid.parse::<ShutdownGrace>().is_err(), "{invalid}");
        }
    }

    #[test]
    fn first_drain_fixes_the_deadline() {
        let clock = DrainClock::new("1s".parse().unwrap());
        assert_eq!(clock.begin(), clock.begin());
    }
}
