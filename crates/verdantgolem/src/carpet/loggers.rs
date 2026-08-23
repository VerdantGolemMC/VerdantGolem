//! Carpet-style `/log` subscriptions: repeating action-bar readouts.
//!
//! Two loggers exist: `tps` (server TPS/MSPT) and `mobcaps` (live mob cap
//! counts for the subscriber's world). Subscriptions are per player UUID and
//! update once per second; entries of offline players are dropped lazily.

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex, Weak},
};

use uuid::Uuid;
use verdantgolem_data::entity::MobCategory;

use crate::{TextComponent, entity::player::Player, server::Server};

/// Update cadence in ticks (once per second).
const LOG_INTERVAL: u64 = 20;

/// Vanilla mob cap scaling: chunks are 17x17 sections.
const MAGIC_NUMBER: i32 = 289;

#[derive(Clone, Copy)]
pub enum Logger {
    Tps,
    MobCaps,
}

#[derive(Default)]
struct Subscriptions {
    tps: HashMap<Uuid, Weak<Player>>,
    mobcaps: HashMap<Uuid, Weak<Player>>,
}

static SUBSCRIPTIONS: LazyLock<Mutex<Subscriptions>> =
    LazyLock::new(|| Mutex::new(Subscriptions::default()));

/// Toggles a logger for `player`, returning the new state (true = subscribed).
pub fn toggle(logger: Logger, player: &Arc<Player>) -> bool {
    let mut subs = SUBSCRIPTIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let map = match logger {
        Logger::Tps => &mut subs.tps,
        Logger::MobCaps => &mut subs.mobcaps,
    };
    match map.remove(&player.gameprofile.id) {
        Some(_) => false,
        None => {
            map.insert(player.gameprofile.id, Arc::downgrade(player));
            true
        }
    }
}

/// Clears every logger subscription of `player`.
pub fn clear(player: &Arc<Player>) {
    let mut subs = SUBSCRIPTIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    subs.tps.remove(&player.gameprofile.id);
    subs.mobcaps.remove(&player.gameprofile.id);
}

fn cap_line(category: &MobCategory, count: i32, spawnable_chunks: i32, multiplier: f64) -> String {
    let cap = (f64::from(category.max) * multiplier) as i32 * spawnable_chunks / MAGIC_NUMBER;
    format!("{}: {count}/{cap}", logger_category_name(category))
}

fn logger_category_name(category: &MobCategory) -> &'static str {
    match category.id {
        0 => "monster",
        1 => "creature",
        2 => "ambient",
        3 => "axolotls",
        4 => "underground_water",
        5 => "water_creature",
        6 => "water_ambient",
        _ => "misc",
    }
}

/// Sends the subscribed readouts; called every server tick.
pub async fn tick_loggers(server: &Arc<Server>, tick_number: u64) {
    if tick_number % LOG_INTERVAL != 0 {
        return;
    }

    let mut targets: Vec<(Logger, Arc<Player>)> = Vec::new();
    if let Ok(mut subs) = SUBSCRIPTIONS.lock() {
        subs.tps.retain(|_, weak| weak.strong_count() > 0);
        subs.mobcaps.retain(|_, weak| weak.strong_count() > 0);
        for (logger, map) in [(Logger::Tps, &subs.tps), (Logger::MobCaps, &subs.mobcaps)] {
            for weak in map.values() {
                if let Some(player) = weak.upgrade() {
                    targets.push((logger, player));
                }
            }
        }
    }

    let multiplier = crate::carpet::values().mob_cap_multiplier;
    for (logger, player) in targets {
        let message = match logger {
            Logger::Tps => TextComponent::text(format!(
                "TPS: {:.1} | MSPT: {:.1}",
                server.get_tps(),
                server.get_mspt()
            )),
            Logger::MobCaps => {
                let world = player.world();
                let state = world.spawn_state.load();
                let spawnable = state.spawnable_chunk_count();
                let lines = [
                    cap_line(
                        &MobCategory::MONSTER,
                        state.category_count(&MobCategory::MONSTER),
                        spawnable,
                        multiplier,
                    ),
                    cap_line(
                        &MobCategory::CREATURE,
                        state.category_count(&MobCategory::CREATURE),
                        spawnable,
                        multiplier,
                    ),
                ];
                TextComponent::text(format!("Mobcaps | {}", lines.join(" | ")))
            }
        };
        player.send_system_message_raw(&message, true).await;
    }
}
