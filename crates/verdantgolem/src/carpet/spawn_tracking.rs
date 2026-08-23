//! `/spawn tracking`: samples successful natural mob spawns while active.
//!
//! The entire tracking session is protected by one lock. This makes starting,
//! stopping and taking the final report one atomic state transition and keeps
//! samples from different dimensions separate.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use verdantgolem_data::entity::EntityType;
use verdantgolem_data::tag::Taggable;

#[derive(Default)]
struct DimensionStats {
    total: u64,
    counts: BTreeMap<String, u64>,
}

struct Session {
    started_at: Instant,
    dimensions: BTreeMap<String, DimensionStats>,
}

#[derive(Default)]
struct Tracker {
    session: Option<Session>,
}

/// Result of atomically toggling spawn tracking.
pub enum ToggleResult {
    Started,
    Stopped(String),
}

impl Tracker {
    fn toggle(&mut self, now: Instant) -> ToggleResult {
        if let Some(session) = self.session.take() {
            ToggleResult::Stopped(format_report(&session, now))
        } else {
            self.session = Some(Session {
                started_at: now,
                dimensions: BTreeMap::new(),
            });
            ToggleResult::Started
        }
    }

    fn record(&mut self, dimension: &str, entity_name: &str) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let stats = session.dimensions.entry(dimension.to_owned()).or_default();
        stats.total = stats.total.saturating_add(1);
        let count = stats.counts.entry(entity_name.to_owned()).or_default();
        *count = count.saturating_add(1);
    }
}

static TRACKER: LazyLock<Mutex<Tracker>> = LazyLock::new(|| Mutex::new(Tracker::default()));

/// Records one successfully inserted natural spawn; no-op unless tracking is active.
pub fn record(dimension: &str, entity_type: &EntityType) {
    TRACKER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .record(dimension, entity_type.registry_key());
}

/// Atomically starts a fresh session or stops the current one and returns its report.
#[must_use]
pub fn toggle() -> ToggleResult {
    TRACKER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .toggle(Instant::now())
}

/// Whether tracking is currently running.
#[must_use]
pub fn is_active() -> bool {
    TRACKER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .session
        .is_some()
}

fn spawn_rate(total: u64, elapsed: Duration) -> f64 {
    let seconds = elapsed.as_secs_f64();
    if seconds > 0.0 {
        total as f64 / seconds
    } else {
        0.0
    }
}

fn format_report(session: &Session, now: Instant) -> String {
    let elapsed = now
        .checked_duration_since(session.started_at)
        .unwrap_or_default();
    let total = session
        .dimensions
        .values()
        .fold(0u64, |sum, stats| sum.saturating_add(stats.total));
    let mut message = format!(
        "Spawn tracking: {total} spawns in {:.1}s ({:.1}/s)",
        elapsed.as_secs_f64(),
        spawn_rate(total, elapsed)
    );

    if session.dimensions.is_empty() {
        message.push_str("\n(no natural spawns recorded)");
        return message;
    }

    for (dimension, stats) in &session.dimensions {
        let _ = write!(
            message,
            "\n[{dimension}] {} spawns ({:.1}/s)",
            stats.total,
            spawn_rate(stats.total, elapsed)
        );
        let mut entries: Vec<_> = stats.counts.iter().collect();
        entries.sort_by(|(name_a, count_a), (name_b, count_b)| {
            count_b.cmp(count_a).then_with(|| name_a.cmp(name_b))
        });
        let omitted = entries.len().saturating_sub(10);
        for (name, count) in entries.into_iter().take(10) {
            let _ = write!(message, "\n  {name}: {count}");
        }
        if omitted > 0 {
            let _ = write!(message, "\n  ... and {omitted} more entity types");
        }
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_is_atomic_and_dimension_scoped() {
        let start = Instant::now();
        let mut tracker = Tracker::default();
        assert!(matches!(tracker.toggle(start), ToggleResult::Started));

        tracker.record("minecraft:overworld", "minecraft:zombie");
        tracker.record("minecraft:overworld", "minecraft:zombie");
        tracker.record("minecraft:the_nether", "minecraft:zombified_piglin");

        let ToggleResult::Stopped(report) = tracker.toggle(start + Duration::from_secs(2)) else {
            panic!("active session should stop");
        };
        assert!(report.contains("Spawn tracking: 3 spawns in 2.0s (1.5/s)"));
        assert!(report.contains("[minecraft:overworld] 2 spawns (1.0/s)"));
        assert!(report.contains("minecraft:zombie: 2"));
        assert!(report.contains("[minecraft:the_nether] 1 spawns (0.5/s)"));

        assert!(matches!(
            tracker.toggle(start + Duration::from_secs(3)),
            ToggleResult::Started
        ));
        let ToggleResult::Stopped(new_report) = tracker.toggle(start + Duration::from_secs(4))
        else {
            panic!("new session should stop");
        };
        assert!(new_report.contains("Spawn tracking: 0 spawns"));
        assert!(!new_report.contains("zombie"));
    }

    #[test]
    fn inactive_tracker_ignores_records() {
        let mut tracker = Tracker::default();
        tracker.record("minecraft:overworld", "minecraft:zombie");
        assert!(tracker.session.is_none());
    }

    #[test]
    fn report_marks_truncated_entity_types() {
        let start = Instant::now();
        let mut tracker = Tracker::default();
        assert!(matches!(tracker.toggle(start), ToggleResult::Started));
        for index in 0..12 {
            tracker.record("minecraft:overworld", &format!("minecraft:mob_{index}"));
        }
        let ToggleResult::Stopped(report) = tracker.toggle(start + Duration::from_secs(1)) else {
            panic!("active session should stop");
        };
        assert!(report.contains("... and 2 more entity types"));
    }
}
