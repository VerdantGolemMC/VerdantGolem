//! Sixteen-channel hopper counters, one per dye color (carpet `hopperCounters`).
//!
//! When the `hopperCounters` rule is enabled, a hopper that transfers items into a
//! wool block destroys the items and counts them on the channel matching the wool's
//! dye color. Counters live in memory only and reset on server restart, like carpet.
//!
//! Rates use either simulated time (20 game ticks per second, independent of
//! `/tick rate`) or monotonic real time (including lag and time spent frozen).
//! All dimensions share one game clock, advanced once by `Server::tick_worlds`;
//! their independent world ages must never be used as counter timestamps.

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use verdantgolem_data::Block;

const GAME_TICKS_PER_SECOND: u64 = 20;

#[derive(Clone, Copy)]
struct SampleTime {
    game_ticks: u64,
    realtime: Instant,
}

#[derive(Default)]
struct Channel {
    items: BTreeMap<String, u64>,
    started_at: Option<SampleTime>,
}

#[derive(Default)]
struct CounterStore {
    /// One channel per dye color, indexed by `DyeColor::id()`.
    channels: [Channel; 16],
    game_ticks: u64,
}

static COUNTERS: LazyLock<Mutex<CounterStore>> =
    LazyLock::new(|| Mutex::new(CounterStore::default()));

/// The clock used for a counter's production rate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeBasis {
    /// Simulated time at 20 TPS; frozen ticks do not advance this clock.
    GameTime,
    /// Actual elapsed time, including freezes, lag and changed tick rates.
    RealTime,
}

/// An atomic snapshot of a channel's counts and both measurement clocks.
#[derive(Debug, Default)]
pub struct CounterReport {
    /// `(item, count)` pairs, sorted by descending count then ascending item key.
    pub items: Vec<(String, u64)>,
    pub total: u64,
    /// Whether the channel has received an item or was explicitly reset.
    pub started: bool,
    pub elapsed_ticks: u64,
    pub elapsed_realtime: Duration,
}

impl CounterReport {
    #[must_use]
    pub fn elapsed(&self, basis: TimeBasis) -> Duration {
        match basis {
            TimeBasis::GameTime => {
                // Split before multiplying so even a saturated tick clock is safe.
                Duration::from_secs(self.elapsed_ticks / GAME_TICKS_PER_SECOND)
                    + Duration::from_millis((self.elapsed_ticks % GAME_TICKS_PER_SECOND) * 50)
            }
            TimeBasis::RealTime => self.elapsed_realtime,
        }
    }

    /// Items/hour over the whole channel session, for either its total or an item.
    /// `None` means no measurement has started or no time has elapsed yet.
    #[must_use]
    pub fn items_per_hour(&self, count: u64, basis: TimeBasis) -> Option<f64> {
        let seconds = self.elapsed(basis).as_secs_f64();
        (self.started && seconds > 0.0).then(|| count as f64 * 3600.0 / seconds)
    }
}

impl CounterStore {
    const fn advance_game_tick(&mut self) {
        self.game_ticks = self.game_ticks.saturating_add(1);
    }

    fn add<'a>(
        &mut self,
        channel: usize,
        items: impl IntoIterator<Item = (&'a str, u64)>,
        now: Instant,
    ) {
        let Some(channel) = self.channels.get_mut(channel) else {
            return;
        };
        for (item_key, count) in items {
            if count == 0 {
                continue;
            }
            channel.started_at.get_or_insert(SampleTime {
                game_ticks: self.game_ticks,
                realtime: now,
            });
            let total = channel.items.entry(item_key.to_owned()).or_default();
            *total = total.saturating_add(count);
        }
    }

    fn report(&self, channel: usize, now: Instant) -> CounterReport {
        let Some(channel) = self.channels.get(channel) else {
            return CounterReport::default();
        };
        let Some(started_at) = channel.started_at else {
            return CounterReport::default();
        };
        let mut items: Vec<_> = channel.items.iter().map(|(k, v)| (k.clone(), *v)).collect();
        items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let total = items
            .iter()
            .fold(0u64, |sum, (_, count)| sum.saturating_add(*count));
        CounterReport {
            items,
            total,
            started: true,
            elapsed_ticks: self.game_ticks.saturating_sub(started_at.game_ticks),
            elapsed_realtime: now.saturating_duration_since(started_at.realtime),
        }
    }

    fn reports(&self, now: Instant) -> [CounterReport; 16] {
        std::array::from_fn(|channel| self.report(channel, now))
    }

    fn reset(&mut self, channel: Option<usize>, now: Instant) {
        let started_at = Some(SampleTime {
            game_ticks: self.game_ticks,
            realtime: now,
        });
        let reset_channel = |channel: &mut Channel| {
            channel.items.clear();
            channel.started_at = started_at;
        };
        if let Some(channel) = channel {
            if let Some(channel) = self.channels.get_mut(channel) {
                reset_channel(channel);
            }
        } else {
            self.channels.iter_mut().for_each(reset_channel);
        }
    }
}

/// Wool blocks in dye-color order; the array index is the counter channel.
const WOOL_BLOCKS: [Block; 16] = [
    Block::WHITE_WOOL,
    Block::ORANGE_WOOL,
    Block::MAGENTA_WOOL,
    Block::LIGHT_BLUE_WOOL,
    Block::YELLOW_WOOL,
    Block::LIME_WOOL,
    Block::PINK_WOOL,
    Block::GRAY_WOOL,
    Block::LIGHT_GRAY_WOOL,
    Block::CYAN_WOOL,
    Block::PURPLE_WOOL,
    Block::BLUE_WOOL,
    Block::BROWN_WOOL,
    Block::GREEN_WOOL,
    Block::RED_WOOL,
    Block::BLACK_WOOL,
];

/// The counter channel a wool block feeds, or `None` for non-wool blocks.
#[must_use]
pub fn wool_channel(block: &Block) -> Option<usize> {
    WOOL_BLOCKS.iter().position(|wool| wool == block)
}

/// Records `count` items of `item_key` on `channel`.
pub fn add(channel: usize, item_key: &str, count: u64) {
    add_batch(channel, [(item_key, count)]);
}

/// Records one hopper transfer atomically, using the same timestamp for all slots.
/// Empty transfers do not start a measurement session.
pub fn add_batch<'a>(channel: usize, items: impl IntoIterator<Item = (&'a str, u64)>) {
    let mut counters = lock(&COUNTERS);
    counters.add(channel, items, Instant::now());
}

/// Advances the shared simulated clock once per actual world tick, not per
/// dimension or networking tick. Call only from `Server::tick_worlds`.
pub fn advance_game_tick() {
    lock(&COUNTERS).advance_game_tick();
}

fn lock(counters: &Mutex<CounterStore>) -> std::sync::MutexGuard<'_, CounterStore> {
    counters
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Snapshot of one channel as `(item, count)` pairs, largest count first.
#[must_use]
pub fn snapshot(channel: usize) -> Vec<(String, u64)> {
    report(channel).items
}

/// Total items counted on one channel.
#[must_use]
pub fn total(channel: usize) -> u64 {
    report(channel).total
}

/// Counts and measurement durations captured under the same lock.
#[must_use]
pub fn report(channel: usize) -> CounterReport {
    let counters = lock(&COUNTERS);
    counters.report(channel, Instant::now())
}

/// Snapshots every channel against a single pair of clock readings.
#[must_use]
pub fn reports() -> [CounterReport; 16] {
    let counters = lock(&COUNTERS);
    counters.reports(Instant::now())
}

/// Clears counts and starts a fresh timed measurement immediately for one
/// channel, or every channel when `channel` is `None`.
pub fn reset(channel: Option<usize>) {
    let mut counters = lock(&COUNTERS);
    counters.reset(channel, Instant::now());
}

/// The 16 channel names, in `DyeColor::id()` order.
pub const CHANNEL_NAMES: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];

/// Resolves a channel name (or `#<index>`) to its channel index.
#[must_use]
pub fn channel_from_name(name: &str) -> Option<usize> {
    CHANNEL_NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .or_else(|| {
            name.strip_prefix('#')
                .and_then(|i| i.parse::<usize>().ok())
                .filter(|index| *index < CHANNEL_NAMES.len())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_names_round_trip() {
        for (index, name) in CHANNEL_NAMES.iter().enumerate() {
            assert_eq!(channel_from_name(name), Some(index));
        }
        assert_eq!(channel_from_name("#3"), Some(3));
        assert_eq!(channel_from_name("#15"), Some(15));
        assert_eq!(channel_from_name("#16"), None);
        assert_eq!(channel_from_name("diamond"), None);
    }

    #[test]
    fn wool_channels_cover_all_sixteen_colors() {
        for (index, block) in [
            Block::WHITE_WOOL,
            Block::ORANGE_WOOL,
            Block::MAGENTA_WOOL,
            Block::LIGHT_BLUE_WOOL,
            Block::YELLOW_WOOL,
            Block::LIME_WOOL,
            Block::PINK_WOOL,
            Block::GRAY_WOOL,
            Block::LIGHT_GRAY_WOOL,
            Block::CYAN_WOOL,
            Block::PURPLE_WOOL,
            Block::BLUE_WOOL,
            Block::BROWN_WOOL,
            Block::GREEN_WOOL,
            Block::RED_WOOL,
            Block::BLACK_WOOL,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(wool_channel(&block), Some(index));
        }
        assert_eq!(wool_channel(&Block::STONE), None);
    }

    #[test]
    fn add_snapshot_and_reset() {
        let now = Instant::now();
        let mut counters = CounterStore::default();
        counters.add(5, [("minecraft:cobblestone", 7)], now);
        counters.add(5, [("minecraft:diamond", 1)], now);
        counters.add(5, [("minecraft:cobblestone", 3)], now);

        let report = counters.report(5, now);
        assert_eq!(report.total, 11);
        let items = report.items;
        assert_eq!(items[0], ("minecraft:cobblestone".to_string(), 10));
        assert_eq!(items.len(), 2);

        counters.reset(Some(5), now);
        let report = counters.report(5, now);
        assert_eq!(report.total, 0);
        assert!(report.items.is_empty());
    }

    #[test]
    fn counter_arithmetic_saturates() {
        let now = Instant::now();
        let mut counters = CounterStore::default();
        counters.add(15, [("minecraft:stone", u64::MAX)], now);
        counters.add(15, [("minecraft:stone", 1)], now);
        counters.add(15, [("minecraft:dirt", 1)], now);

        let report = counters.report(15, now);
        assert_eq!(report.items[0].1, u64::MAX);
        assert_eq!(report.total, u64::MAX);
    }

    #[test]
    fn first_nonzero_transfer_starts_both_clocks() {
        let start = Instant::now();
        let mut counters = CounterStore::default();
        counters.add(0, [("minecraft:stone", 0)], start);
        counters.advance_game_tick();
        let empty = counters.report(0, start + Duration::from_secs(30));
        assert!(!empty.started);
        assert_eq!(empty.elapsed_ticks, 0);
        assert_eq!(empty.items_per_hour(0, TimeBasis::GameTime), None);

        let first_item = start + Duration::from_mins(1);
        counters.add(0, [("minecraft:stone", 1)], first_item);
        let report = counters.report(0, first_item);
        assert!(report.started);
        assert_eq!(report.elapsed_ticks, 0);
        assert_eq!(report.elapsed_realtime, Duration::ZERO);
        for basis in [TimeBasis::GameTime, TimeBasis::RealTime] {
            assert_eq!(report.items_per_hour(report.total, basis), None);
        }
    }

    #[test]
    fn all_items_and_dimensions_share_the_channel_session_clock() {
        let start = Instant::now();
        let mut counters = CounterStore::default();
        counters.reset(Some(0), start);
        // Hoppers in different dimensions contribute to one channel. Recording
        // items does not advance time, regardless of the number of dimensions.
        counters.add(0, [("minecraft:stone", 20), ("minecraft:dirt", 10)], start);
        counters.add(0, [("minecraft:stone", 10), ("minecraft:dirt", 20)], start);
        assert_eq!(counters.game_ticks, 0);
        for _ in 0..1200 {
            counters.advance_game_tick();
        }
        let report = counters.report(0, start + Duration::from_mins(1));
        assert_eq!(report.elapsed_ticks, 1200);
        assert_eq!(report.total, 60);
        // Ties are sorted by item key, not transfer order or dimension.
        assert_eq!(report.items[0], ("minecraft:dirt".to_string(), 30));
        assert_eq!(report.items[1], ("minecraft:stone".to_string(), 30));
        for basis in [TimeBasis::GameTime, TimeBasis::RealTime] {
            assert_eq!(report.items_per_hour(report.total, basis), Some(3600.0));
            assert_eq!(
                report.items_per_hour(report.items[0].1, basis),
                Some(1800.0)
            );
        }
    }

    #[test]
    fn freeze_and_step_advance_only_the_appropriate_clock() {
        let start = Instant::now();
        let mut counters = CounterStore::default();
        counters.reset(Some(0), start);
        counters.add(0, [("minecraft:stone", 20)], start);
        for _ in 0..20 {
            counters.advance_game_tick();
        }
        let running = counters.report(0, start + Duration::from_secs(1));
        assert_eq!(
            running.items_per_hour(20, TimeBasis::GameTime),
            Some(72_000.0)
        );
        assert_eq!(
            running.items_per_hour(20, TimeBasis::RealTime),
            Some(72_000.0)
        );

        // Network-only ticks during `/tick freeze` never call advance_game_tick.
        let frozen = counters.report(0, start + Duration::from_secs(10));
        assert_eq!(frozen.elapsed_ticks, 20);
        assert_eq!(
            frozen.items_per_hour(20, TimeBasis::GameTime),
            Some(72_000.0)
        );
        assert_eq!(frozen.items_per_hour(20, TimeBasis::RealTime), Some(7200.0));

        // `/tick step 1` enters tick_worlds exactly once, even while frozen.
        counters.advance_game_tick();
        let stepped = counters.report(0, start + Duration::from_millis(10_050));
        assert_eq!(
            stepped.elapsed(TimeBasis::GameTime),
            Duration::from_millis(1050)
        );
        assert_eq!(
            stepped.elapsed(TimeBasis::RealTime),
            Duration::from_millis(10_050)
        );
    }

    #[test]
    fn tick_rate_and_sprint_change_realtime_but_not_normalized_game_rates() {
        let start = Instant::now();
        let mut counters = CounterStore::default();
        counters.add(0, [("minecraft:stone", 100)], start);
        for _ in 0..200 {
            counters.advance_game_tick();
        }
        // The same 200 simulated ticks may take 20s at low TPS or 2s in a sprint.
        for (wall_seconds, expected_rate) in [(20, 18_000.0), (2, 180_000.0)] {
            let report = counters.report(0, start + Duration::from_secs(wall_seconds));
            assert_eq!(report.elapsed(TimeBasis::GameTime), Duration::from_secs(10));
            assert_eq!(
                report.items_per_hour(100, TimeBasis::GameTime),
                Some(36_000.0)
            );
            assert_eq!(
                report.items_per_hour(100, TimeBasis::RealTime),
                Some(expected_rate)
            );
        }
    }

    #[test]
    fn reset_one_and_all_restart_clocks_without_resetting_the_shared_tick() {
        let start = Instant::now();
        let mut counters = CounterStore::default();
        counters.add(0, [("minecraft:stone", 3)], start);
        counters.add(1, [("minecraft:dirt", 7)], start);
        for _ in 0..20 {
            counters.advance_game_tick();
        }
        let reset_time = start + Duration::from_secs(1);
        counters.reset(Some(0), reset_time);
        assert_eq!(counters.game_ticks, 20);
        let cleared = counters.report(0, reset_time);
        assert!(cleared.started);
        assert_eq!(cleared.total, 0);
        assert!(cleared.items.is_empty());
        assert_eq!(cleared.elapsed_ticks, 0);
        assert_eq!(cleared.elapsed_realtime, Duration::ZERO);
        assert_eq!(counters.report(1, reset_time).total, 7);
        assert_eq!(counters.report(1, reset_time).elapsed_ticks, 20);

        counters.advance_game_tick();
        let next_tick = reset_time + Duration::from_millis(50);
        // Explicit reset starts measurement immediately, including zero output.
        let idle = counters.report(0, next_tick);
        assert_eq!(idle.items_per_hour(0, TimeBasis::GameTime), Some(0.0));
        counters.add(0, [("minecraft:stone", 1)], next_tick);
        assert_eq!(counters.report(0, next_tick).elapsed_ticks, 1);

        counters.reset(None, next_tick);
        assert_eq!(counters.game_ticks, 21);
        for channel in 0..16 {
            let report = counters.report(channel, next_tick);
            assert!(report.started);
            assert!(report.items.is_empty());
            assert_eq!(report.total, 0);
            assert_eq!(report.elapsed_ticks, 0);
            assert_eq!(report.elapsed_realtime, Duration::ZERO);
        }
    }

    #[test]
    fn all_channel_reports_use_one_pair_of_clock_readings() {
        let start = Instant::now();
        let mut counters = CounterStore::default();
        counters.reset(None, start);
        counters.add(0, [("minecraft:stone", 3)], start);
        counters.add(1, [("minecraft:dirt", 7)], start);
        for _ in 0..20 {
            counters.advance_game_tick();
        }

        let reports = counters.reports(start + Duration::from_secs(2));
        assert_eq!(reports[0].total, 3);
        assert_eq!(reports[1].total, 7);
        for report in reports {
            assert!(report.started);
            assert_eq!(report.elapsed_ticks, 20);
            assert_eq!(report.elapsed_realtime, Duration::from_secs(2));
        }
    }

    #[test]
    fn concurrent_batches_resets_and_reports_never_expose_partial_transfers() {
        let counters = Mutex::new(CounterStore::default());
        let barrier = std::sync::Barrier::new(4);
        std::thread::scope(|scope| {
            for _ in 0..2 {
                scope.spawn(|| {
                    barrier.wait();
                    for _ in 0..256 {
                        let mut counters = lock(&counters);
                        counters.add(
                            0,
                            [("minecraft:stone", 2), ("minecraft:dirt", 1)],
                            Instant::now(),
                        );
                    }
                });
            }
            scope.spawn(|| {
                barrier.wait();
                for _ in 0..256 {
                    let mut counters = lock(&counters);
                    counters.reset(Some(0), Instant::now());
                    counters.advance_game_tick();
                }
            });

            barrier.wait();
            for _ in 0..256 {
                let counters = lock(&counters);
                let now = Instant::now();
                let report = counters.report(0, now);
                assert_eq!(report.total % 3, 0);
                if report.total > 0 {
                    assert_eq!(report.items.len(), 2);
                    assert_eq!(report.items[0].1, report.total / 3 * 2);
                    assert_eq!(report.items[1].1, report.total / 3);
                }
                if let Some(started) = counters.channels[0].started_at {
                    assert!(report.started);
                    assert_eq!(
                        report.elapsed_ticks,
                        counters.game_ticks - started.game_ticks
                    );
                    assert_eq!(
                        report.elapsed_realtime,
                        now.duration_since(started.realtime)
                    );
                }
            }
        });
    }

    #[test]
    fn invalid_channels_and_regressing_sample_times_are_harmless() {
        let start = Instant::now();
        let mut counters = CounterStore::default();
        counters.add(16, [("minecraft:stone", 1)], start);
        counters.reset(Some(usize::MAX), start);
        assert!(!counters.report(16, start).started);
        assert!(
            counters
                .channels
                .iter()
                .all(|channel| channel.started_at.is_none())
        );

        counters.add(0, [("minecraft:stone", 1)], start + Duration::from_secs(1));
        let earlier = counters.report(0, start);
        assert_eq!(earlier.elapsed_realtime, Duration::ZERO);
        assert_eq!(earlier.items_per_hour(1, TimeBasis::RealTime), None);
    }

    #[test]
    fn saturated_clocks_and_counts_keep_rates_finite() {
        let start = Instant::now();
        let mut counters = CounterStore {
            game_ticks: u64::MAX - 1,
            ..CounterStore::default()
        };
        counters.add(
            0,
            [
                ("minecraft:stone", u64::MAX),
                ("minecraft:stone", 1),
                ("minecraft:dirt", 1),
            ],
            start,
        );
        counters.advance_game_tick();
        counters.advance_game_tick();
        assert_eq!(counters.game_ticks, u64::MAX);
        let mut report = counters.report(0, start + Duration::from_nanos(1));
        assert_eq!(report.total, u64::MAX);
        assert_eq!(report.elapsed_ticks, 1);
        for basis in [TimeBasis::GameTime, TimeBasis::RealTime] {
            assert!(
                report
                    .items_per_hour(report.total, basis)
                    .is_some_and(f64::is_finite)
            );
        }
        report.elapsed_ticks = u64::MAX;
        assert!(
            report
                .items_per_hour(report.total, TimeBasis::GameTime)
                .is_some_and(f64::is_finite)
        );
    }

    #[test]
    fn poisoned_lock_still_allows_recording_reading_and_resetting() {
        let start = Instant::now();
        let counters = Mutex::new(CounterStore::default());
        let poisoned = std::panic::catch_unwind(|| {
            let mut store = lock(&counters);
            store.add(0, [("minecraft:stone", 1)], start);
            panic!("simulate a previous holder panicking");
        });
        assert!(poisoned.is_err());
        assert!(counters.is_poisoned());
        let mut store = lock(&counters);
        store.add(0, [("minecraft:stone", 1)], start);
        store.advance_game_tick();
        assert_eq!(store.report(0, start).total, 2);
        store.reset(Some(0), start);
        assert_eq!(store.report(0, start).total, 0);
        store.add(1, [("minecraft:dirt", 1)], start);
        store.reset(None, start);
        drop(store);
        assert_eq!(lock(&counters).report(1, start).total, 0);
    }
}
