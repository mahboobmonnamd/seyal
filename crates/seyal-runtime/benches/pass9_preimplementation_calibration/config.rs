use std::time::Duration;

pub(crate) const WARMUP_CYCLES: usize = 20;
pub(crate) const MEASURED_CYCLES: usize = 100;
// The raw series remains authoritative. This trailing window makes a retained
// allocator high-water mark distinguishable from RSS that is still advancing
// at the end of a cohort; it must never replace the full-series slope/RCA.
pub(crate) const RSS_TAIL_CYCLES: usize = 25;
pub(crate) const IDLE_CPU_SAMPLE_COUNT: usize = 5;
pub(crate) const IDLE_CPU_SAMPLE_WINDOW: Duration = Duration::from_millis(250);
pub(crate) const COHORTS: usize = 5;
pub(crate) const QUIESCENT_SAMPLE_COUNT: usize = 5;
pub(crate) const QUIESCENT_SAMPLE_INTERVAL: Duration = Duration::from_millis(10);
pub(crate) const SETTLE_TIMEOUT: Duration = Duration::from_secs(2);
// Bounds on every blocking wait on a spawned process. A calibration run that
// cannot make progress must fail loudly within minutes, not hang until an
// external CI job timeout kills it silently with no diagnostic (as observed:
// an unrelated panic left an orphaned --runtime-worker holding open the
// inherited stderr pipe, and the parent's blocking read waited forever).
pub(crate) const WORKER_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const WORKER_RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const COHORT_PROCESS_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LossMode {
    Graceful,
    Abrupt,
}

impl LossMode {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Graceful => "graceful_detach",
            Self::Abrupt => "abrupt_socket_loss",
        }
    }

    pub(crate) fn parse(value: &str) -> Self {
        match value {
            "graceful_detach" => Self::Graceful,
            "abrupt_socket_loss" => Self::Abrupt,
            _ => panic!("unknown Pass 9 loss mode: {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Geometry {
    pub(crate) columns: u16,
    pub(crate) rows: u16,
}

impl Geometry {
    pub(crate) const PRIMARY: Self = Self {
        columns: 120,
        rows: 40,
    };
    pub(crate) const REPRESENTATIVE: Self = Self {
        columns: 80,
        rows: 24,
    };

    pub(crate) fn label(self) -> String {
        format!("{}x{}", self.columns, self.rows)
    }

    pub(crate) fn parse(value: &str) -> Self {
        let (columns, rows) = value.split_once('x').expect("geometry columnsxrows");
        Self {
            columns: columns.parse().expect("geometry columns"),
            rows: rows.parse().expect("geometry rows"),
        }
    }
}
