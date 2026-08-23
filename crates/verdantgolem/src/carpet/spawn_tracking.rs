//! `/spawn tracking`: samples natural mob spawns while active (carpet-style).
//!
//! Recording hooks into the natural spawner's successful spawn path; the
//! report shows duration, totals, per-type counts and spawns per second.

use std::{
    collections::BTreeMap,
    fmt::Write as _,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
    },
};

use verdantgolem_data::entity::EntityType;
use verdantgolem_data::tag::Taggable;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static TOTAL: AtomicU64 = AtomicU64::new(0);
static STARTED_AT: AtomicI64 = AtomicI64::new(0);
static COUNTS: LazyLock<Mutex<BTreeMap<String, u64>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

/// Records one naturally spawned entity; no-op unless tracking is active.
pub fn record(entity_type: &EntityType) {
    if !ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    TOTAL.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut counts) = COUNTS.lock() {
        *counts
            .entry(entity_type.registry_key().to_string())
            .or_insert(0) += 1;
    }
}

/// Toggles tracking; starting resets previous samples.
/// Returns `true` when tracking is now active.
pub fn toggle() -> bool {
    if ACTIVE.swap(false, Ordering::Relaxed) {
        return false;
    }
    TOTAL.store(0, Ordering::Relaxed);
    if let Ok(mut counts) = COUNTS.lock() {
        counts.clear();
    }
    STARTED_AT.store(now_seconds(), Ordering::Relaxed);
    ACTIVE.store(true, Ordering::Relaxed);
    true
}

/// Whether tracking is currently running.
#[must_use]
pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
}

/// Human-readable report of the current or last samples.
#[must_use]
pub fn report() -> String {
    let elapsed = (now_seconds() - STARTED_AT.load(Ordering::Relaxed)).max(1);
    let total = TOTAL.load(Ordering::Relaxed);
    let mut message = format!(
        "Spawn tracking: {total} spawns in {elapsed}s ({:.1}/s)",
        total as f64 / elapsed as f64
    );
    if let Ok(counts) = COUNTS.lock() {
        let mut entries: Vec<(&String, &u64)> = counts.iter().collect();
        entries.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
        for (name, count) in entries.iter().take(10) {
            let _ = writeln!(message, "{name}: {count}");
        }
    }
    message
}
