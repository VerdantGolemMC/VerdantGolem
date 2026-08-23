//! Carpet-style fake players (`/player`), backed by a headless
//! [`ClientPlatform::Local`] connection.
//!
//! They load chunks, appear in the tab list and to other players, keep farms
//! running while nobody is online, but have no network connection and never
//! receive packets.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use uuid::Uuid;

use crate::{
    entity::player::Player,
    net::{ClientPlatform, GameProfile},
    server::Server,
};
use arc_swap::ArcSwap;
use verdantgolem_data::tag::Taggable;
use verdantgolem_util::Hand;
use verdantgolem_util::math::vector3::Vector3;

/// A fake player plus its repeating-action state.
pub struct FakePlayer {
    player: Arc<Player>,
    /// Continuous attack toggle (`/player <name> attack`).
    attacking: AtomicBool,
    attack_cooldown: AtomicU32,
}

/// Online fake players by name.
static FAKES: LazyLock<Mutex<BTreeMap<String, Arc<FakePlayer>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

/// Ticks between fake-player attacks (~ vanilla sword cooldown).
const ATTACK_INTERVAL: u32 = 12;

/// Vanilla-compatible offline UUID for a fake player name (stable across restarts).
#[must_use]
pub fn offline_uuid(name: &str) -> Uuid {
    Uuid::new_v3(&Uuid::nil(), format!("OfflinePlayer:{name}").as_bytes())
}

/// Whether `name` is a legal Minecraft player name.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    (2..=16).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn profile(name: &str) -> GameProfile {
    GameProfile {
        id: offline_uuid(name),
        name: name.to_string(),
        properties: ArcSwap::from_pointee(Vec::new()),
        profile_actions: None,
    }
}

/// The online fake player named `name`.
#[must_use]
pub fn get(name: &str) -> Option<Arc<Player>> {
    FAKES
        .lock()
        .ok()
        .and_then(|fakes| fakes.get(name).map(|fake| fake.player.clone()))
}

/// Whether a fake player is continuously attacking.
#[must_use]
pub fn is_attacking(name: &str) -> bool {
    FAKES
        .lock()
        .ok()
        .and_then(|fakes| {
            fakes
                .get(name)
                .map(|fake| fake.attacking.load(Ordering::Relaxed))
        })
        .unwrap_or(false)
}

/// Toggles the continuous attack action of a fake player.
pub fn set_attacking(name: &str, on: bool) -> Result<(), String> {
    let fakes = FAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fake = fakes
        .get(name)
        .ok_or_else(|| format!("no fake player named {name}"))?;
    fake.attacking.store(on, Ordering::Relaxed);
    fake.attack_cooldown.store(0, Ordering::Relaxed);
    Ok(())
}

/// Clears all repeating actions of a fake player.
pub fn stop_actions(name: &str) -> Result<(), String> {
    set_attacking(name, false)
}

/// Ticks every fake player in `world`, applying repeating actions.
/// Called once per world tick, so fakes only tick in their own world.
pub fn tick_fakes(world: &Arc<crate::world::World>) {
    let fakes: Vec<Arc<FakePlayer>> = FAKES
        .lock()
        .ok()
        .map(|guard| {
            guard
                .values()
                .filter(|fake| Arc::ptr_eq(&fake.player.world(), world))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    for fake in fakes {
        if !fake.attacking.load(Ordering::Relaxed) {
            continue;
        }
        let cooldown = fake.attack_cooldown.load(Ordering::Relaxed);
        if cooldown > 0 {
            fake.attack_cooldown.store(cooldown - 1, Ordering::Relaxed);
        } else {
            fake.attack_cooldown
                .store(ATTACK_INTERVAL, Ordering::Relaxed);
            try_attack(&fake.player);
        }
    }
}

/// Attacks the closest living entity inside a cone in front of the fake
/// player, like a crosshair attack.
fn try_attack(player: &Arc<Player>) {
    let entity = &player.living_entity.entity;
    let pos = entity.pos.load();
    let yaw = f64::from(entity.yaw.load()).to_radians();
    let pitch = f64::from(entity.pitch.load()).to_radians();
    let dir = Vector3::new(
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    );

    let center = pos + dir.multiply(2.0, 2.0, 2.0);
    let search_box = verdantgolem_util::math::boundingbox::BoundingBox::new(
        center.sub_raw(1.5, 1.5, 1.5),
        center.add_raw(1.5, 1.5, 1.5),
    );
    let world = entity.world.load();

    let mut best: Option<(f64, Arc<dyn crate::entity::EntityBase>)> = None;
    for other in world.get_entities_at_box(&search_box) {
        let other_entity = other.get_entity();
        if other_entity.entity_id == entity.entity_id
            || other_entity.entity_type.id == verdantgolem_data::entity::EntityType::PLAYER.id
        {
            continue;
        }
        if other.get_living_entity().is_none() {
            continue;
        }
        let to = other_entity.pos.load() - pos;
        let dist_sq = to.length_squared();
        if dist_sq > 16.0 || dist_sq < 1.0e-6 {
            continue;
        }
        // Keep targets roughly in front of the fake player.
        let dot = dir.x * to.x + dir.y * to.y + dir.z * to.z;
        if dot / dist_sq.sqrt() < 0.5 {
            continue;
        }
        if best.as_ref().is_none_or(|(best_sq, _)| dist_sq < *best_sq) {
            best = Some((dist_sq, other));
        }
    }

    if let Some((_, target)) = best {
        player.attack(&target);
        player.swing_hand(Hand::Right, true);
    }
}

pub fn list() -> Vec<String> {
    FAKES
        .lock()
        .map(|fakes| fakes.keys().cloned().collect())
        .unwrap_or_default()
}

/// Spawns a fake player named `name` at `position` (with `yaw`/`pitch`) in `world`.
pub async fn spawn(
    world: &Arc<crate::world::World>,
    name: &str,
    position: Vector3<f64>,
    yaw: f32,
    pitch: f32,
) -> Result<(), String> {
    if !valid_name(name) {
        return Err(format!("invalid player name: {name}"));
    }
    if FAKES.lock().is_ok_and(|fakes| fakes.contains_key(name)) {
        return Err(format!("fake player {name} already exists"));
    }
    // Refuse to shadow a real online player.
    if let Some(server) = world.server.upgrade()
        && server.worlds.load().iter().any(|online| {
            online
                .players
                .load()
                .iter()
                .any(|p| p.gameprofile.name.eq_ignore_ascii_case(name))
        })
    {
        return Err(format!("a real player named {name} is online"));
    }

    let player = spawn_untracked(world, name, position, yaw, pitch).await?;
    FAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            name.to_string(),
            Arc::new(FakePlayer {
                player,
                attacking: AtomicBool::new(false),
                attack_cooldown: AtomicU32::new(0),
            }),
        );
    Ok(())
}

async fn spawn_untracked(
    world: &Arc<crate::world::World>,
    name: &str,
    position: Vector3<f64>,
    yaw: f32,
    pitch: f32,
) -> Result<Arc<Player>, String> {
    let Some(server) = world.server.upgrade() else {
        return Err("server is inactive".to_string());
    };
    let Some((player, spawn_world)) = server
        .add_player(Arc::new(ClientPlatform::Local), profile(name), None)
        .await
    else {
        return Err(format!("failed to create fake player {name}"));
    };

    // Position and rotation before broadcasting the spawn.
    let entity = &player.living_entity.entity;
    entity.pos.store(position);
    entity.yaw.store(yaw);
    entity.head_yaw.store(yaw);
    entity.body_yaw.store(yaw);
    entity.pitch.store(pitch);

    // add_player defaults to the first world; move the fake into the
    // sender's world when they differ. All client packets are no-ops for a
    // Local client, so only the world registries need to move.
    if !Arc::ptr_eq(&spawn_world, world) {
        let removed = spawn_world.remove_player(&player, false).await;
        if removed.is_some() {
            _ = world.add_player(&player);
            entity.world.store(world.clone());
        }
    }

    world.spawn_local_player(&player).await;

    Ok(player)
}

/// Removes a fake player by name.
pub async fn kill(server: &Server, name: &str) -> Result<(), String> {
    let player = {
        let mut fakes = FAKES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        fakes
            .remove(name)
            .map(|fake| fake.player.clone())
            .ok_or_else(|| format!("no fake player named {name}"))?
    };

    player.remove().await;
    server.remove_player(&player).await;
    if let Err(e) = server
        .player_data_storage
        .handle_player_leave(&player)
        .await
    {
        return Err(format!("failed to save fake player data: {e}"));
    }
    Ok(())
}

/// Mounts a fake player onto the nearest mountable entity (boats, minecarts,
/// mobs...). Returns a description of the vehicle.
pub async fn mount(name: &str) -> Result<String, String> {
    let player = get(name).ok_or_else(|| format!("no fake player named {name}"))?;
    let entity = &player.living_entity.entity;
    let pos = entity.pos.load();
    let search_box = verdantgolem_util::math::boundingbox::BoundingBox::new(
        pos.sub_raw(3.0, 3.0, 3.0),
        pos.add_raw(3.0, 3.0, 3.0),
    );
    let world = entity.world.load();

    let mut best: Option<(f64, Arc<dyn crate::entity::EntityBase>)> = None;
    for other in world.get_entities_at_box(&search_box) {
        let other_entity = other.get_entity();
        if other_entity.entity_id == entity.entity_id
            || other_entity.entity_type.id == verdantgolem_data::entity::EntityType::PLAYER.id
            || other.get_item_entity().is_some()
        {
            continue;
        }
        let dist_sq = other_entity.pos.load().sub(&pos).length_squared();
        if best.as_ref().is_none_or(|(best_sq, _)| dist_sq < *best_sq) {
            best = Some((dist_sq, other));
        }
    }

    let Some((_, vehicle)) = best else {
        return Err(format!("no mountable entity near {name}"));
    };
    let vehicle_name = vehicle.get_entity().entity_type.registry_key();
    let player_base: Arc<dyn crate::entity::EntityBase> = player.clone();
    vehicle
        .get_entity()
        .add_passenger(vehicle.clone(), player_base)
        .await;

    Ok(vehicle_name.to_string())
}

/// Dismounts a fake player from its vehicle.
pub async fn dismount(name: &str) -> Result<(), String> {
    let player = get(name).ok_or_else(|| format!("no fake player named {name}"))?;
    let entity = &player.living_entity.entity;
    let vehicle = entity.vehicle.lock().await.clone();
    let Some(vehicle) = vehicle else {
        return Err(format!("{name} is not riding anything"));
    };
    vehicle
        .get_entity()
        .remove_passenger(entity.entity_id)
        .await;
    Ok(())
}

/// Turns a fake player in place.
pub fn look_up(player: &Arc<Player>, yaw: f32, pitch: f32) {
    let entity = &player.living_entity.entity;
    entity.yaw.store(yaw);
    entity.head_yaw.store(yaw);
    entity.body_yaw.store(yaw);
    entity.pitch.store(pitch);
    entity.send_pos_rot();
}
