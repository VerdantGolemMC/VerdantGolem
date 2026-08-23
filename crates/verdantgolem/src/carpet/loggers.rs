//! Carpet-style `/log` subscriptions: repeating action-bar readouts.
//!
//! `tps` and `mobcaps` subscriptions are aggregated per player so one logger
//! cannot overwrite another player's action bar update in the same tick.

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, Mutex, Weak},
};

use uuid::Uuid;
use verdantgolem_data::entity::MobCategory;

use crate::{TextComponent, entity::player::Player, server::Server};

/// Update cadence in ticks (once per second).
const LOG_INTERVAL: u64 = 20;
const MAX_DISPLAY_MSPT: f64 = 60_000.0;
const MAX_DISPLAY_TPS: f64 = 1_000.0;

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

#[derive(Default)]
struct SelectedLoggers {
    tps: bool,
    mobcaps: bool,
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
    if map.remove(&player.gameprofile.id).is_some() {
        false
    } else {
        map.insert(player.gameprofile.id, Arc::downgrade(player));
        true
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

fn cap_line(
    category: &'static MobCategory,
    count: i32,
    spawnable_chunks: i32,
    multiplier: f64,
) -> String {
    if category.max < 0 {
        format!("{}: {count}", logger_category_name(category))
    } else {
        let cap = crate::world::natural_spawner::scaled_mob_cap(
            category.max,
            spawnable_chunks,
            multiplier,
        );
        format!("{}: {count}/{cap}", logger_category_name(category))
    }
}

const fn logger_category_name(category: &MobCategory) -> &'static str {
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

fn sanitize_tick_metrics(tps: f64, mspt: f64, configured_tps: f64) -> (f64, f64) {
    let configured_tps = if configured_tps.is_finite() {
        configured_tps.clamp(1.0, MAX_DISPLAY_TPS)
    } else {
        20.0
    };
    let tps = if tps.is_finite() {
        tps.clamp(0.0, configured_tps)
    } else {
        0.0
    };
    let mspt = if mspt.is_finite() {
        mspt.clamp(0.0, MAX_DISPLAY_MSPT)
    } else {
        0.0
    };
    (tps, mspt)
}

fn effective_tps(raw_tps: f64, frozen: bool, runs_normally: bool, sprinting: bool) -> f64 {
    if frozen && !runs_normally && !sprinting {
        0.0
    } else {
        raw_tps
    }
}

/// Sends the subscribed readouts; called every server tick.
pub async fn tick_loggers(server: &Arc<Server>, tick_number: u64) {
    if !tick_number.is_multiple_of(LOG_INTERVAL) {
        return;
    }

    let mut targets: HashMap<Uuid, (Arc<Player>, SelectedLoggers)> = HashMap::new();
    {
        let mut subs = SUBSCRIPTIONS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        subs.tps.retain(|id, weak| {
            let Some(player) = weak.upgrade() else {
                return false;
            };
            targets
                .entry(*id)
                .or_insert_with(|| (player, SelectedLoggers::default()))
                .1
                .tps = true;
            true
        });
        subs.mobcaps.retain(|id, weak| {
            let Some(player) = weak.upgrade() else {
                return false;
            };
            targets
                .entry(*id)
                .or_insert_with(|| (player, SelectedLoggers::default()))
                .1
                .mobcaps = true;
            true
        });
    }

    let multiplier = crate::carpet::values().mob_cap_multiplier;
    let manager = &server.tick_rate_manager;
    let observed_tps = effective_tps(
        server.get_tps(),
        manager.is_frozen(),
        manager.runs_normally(),
        manager.is_sprinting(),
    );
    let metrics = sanitize_tick_metrics(
        observed_tps,
        server.get_mspt(),
        f64::from(manager.tickrate()),
    );
    let messages: Vec<_> = targets
        .into_values()
        .map(|(player, selected)| {
            let mut sections = Vec::with_capacity(2);
            if selected.tps {
                sections.push(format!("TPS: {:.1} | MSPT: {:.1}", metrics.0, metrics.1));
            }
            if selected.mobcaps {
                let world = player.world();
                let state = world.spawn_state.load();
                let spawnable = state.spawnable_chunk_count();
                let lines = MobCategory::SPAWNING_CATEGORIES.map(|category| {
                    cap_line(
                        category,
                        state.category_count(category),
                        spawnable,
                        multiplier,
                    )
                });
                sections.push(format!("Mobcaps | {}", lines.join(" | ")));
            }
            (player, TextComponent::text(sections.join(" || ")))
        })
        .collect();

    futures::future::join_all(messages.into_iter().map(|(player, message)| async move {
        player.send_system_message_raw(&message, true).await;
    }))
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_metrics_are_finite_and_bounded() {
        assert_eq!(
            sanitize_tick_metrics(f64::NAN, f64::INFINITY, 20.0),
            (0.0, 0.0)
        );
        assert_eq!(sanitize_tick_metrics(500.0, -2.0, 20.0), (20.0, 0.0));
        assert_eq!(
            sanitize_tick_metrics(2_000.0, 90_000.0, 5_000.0),
            (1_000.0, 60_000.0)
        );
    }

    #[test]
    fn frozen_server_reports_zero_unless_stepping_or_sprinting() {
        assert_eq!(effective_tps(20.0, true, false, false), 0.0);
        assert_eq!(effective_tps(20.0, true, true, false), 20.0);
        assert_eq!(effective_tps(20.0, true, false, true), 20.0);
    }

    #[test]
    fn mobcap_line_uses_shared_full_precision_formula() {
        assert_eq!(
            cap_line(&MobCategory::MONSTER, 12, 288, 0.51),
            "monster: 12/35"
        );
        assert_eq!(cap_line(&MobCategory::MISC, 3, 288, 1.0), "misc: 3");
    }
}
