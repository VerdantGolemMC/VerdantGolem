//! Sixteen-channel hopper counters, one per dye color (carpet `hopperCounters`).
//!
//! When the `hopperCounters` rule is enabled, a hopper that transfers items into a
//! wool block destroys the items and counts them on the channel matching the wool's
//! dye color. Counters live in memory only and reset on server restart, like carpet.

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};

use verdantgolem_data::Block;

/// One counter channel per dye color, indexed by `DyeColor::id()`.
type Channels = [BTreeMap<String, u64>; 16];

static COUNTERS: LazyLock<Mutex<Channels>> =
    LazyLock::new(|| Mutex::new([const { BTreeMap::new() }; 16]));

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
    if let Ok(mut counters) = COUNTERS.lock()
        && let Some(items) = counters.get_mut(channel)
    {
        *items.entry(item_key.to_string()).or_insert(0) += count;
    }
}

/// Snapshot of one channel as `(item, count)` pairs, largest count first.
#[must_use]
pub fn snapshot(channel: usize) -> Vec<(String, u64)> {
    let Ok(counters) = COUNTERS.lock() else {
        return Vec::new();
    };
    let Some(items) = counters.get(channel) else {
        return Vec::new();
    };
    let mut items: Vec<(String, u64)> = items.iter().map(|(k, v)| (k.clone(), *v)).collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    items
}

/// Total items counted on one channel.
#[must_use]
pub fn total(channel: usize) -> u64 {
    snapshot(channel).iter().map(|(_, count)| count).sum()
}

/// Resets one channel, or every channel when `channel` is `None`.
pub fn reset(channel: Option<usize>) {
    if let Ok(mut counters) = COUNTERS.lock() {
        match channel {
            Some(channel) => {
                if let Some(items) = counters.get_mut(channel) {
                    items.clear();
                }
            }
            None => *counters = [const { BTreeMap::new() }; 16],
        }
    }
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
        .or_else(|| name.strip_prefix('#').and_then(|i| i.parse::<usize>().ok()))
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
        let channel = channel_from_name("lime").unwrap_or(5);
        reset(Some(channel));
        add(channel, "minecraft:cobblestone", 7);
        add(channel, "minecraft:diamond", 1);
        add(channel, "minecraft:cobblestone", 3);

        assert_eq!(total(channel), 11);
        let items = snapshot(channel);
        assert_eq!(items[0], ("minecraft:cobblestone".to_string(), 10));
        assert_eq!(items.len(), 2);

        reset(Some(channel));
        assert_eq!(total(channel), 0);
        assert!(snapshot(channel).is_empty());
    }
}
