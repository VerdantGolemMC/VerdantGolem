//! Carpet-style fake players (`/player`), backed by a headless
//! [`ClientPlatform::Local`] connection: they load chunks, appear in the tab
//! list and to other players, keep farms running while nobody is online, but
//! have no network connection and never receive packets.

use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock, Mutex},
};

use uuid::Uuid;

use crate::{
    entity::player::Player,
    net::{ClientPlatform, GameProfile},
    server::Server,
};
use arc_swap::ArcSwap;
use verdantgolem_util::math::vector3::Vector3;

/// Online fake players by name.
static FAKES: LazyLock<Mutex<BTreeMap<String, Arc<Player>>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

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
    FAKES.lock().ok().and_then(|fakes| fakes.get(name).cloned())
}

/// Names of all online fake players.
#[must_use]
pub fn list() -> Vec<String> {
    FAKES
        .lock()
        .map(|fakes| fakes.keys().cloned().collect())
        .unwrap_or_default()
}

/// Spawns a fake player named `name` at `position` (with `yaw`/`pitch`) in `world`.
pub async fn spawn(
    server: &Arc<Server>,
    world: &Arc<crate::world::World>,
    name: &str,
    position: Vector3<f64>,
    yaw: f32,
    pitch: f32,
) -> Result<(), String> {
    if !valid_name(name) {
        return Err(format!("invalid player name: {name}"));
    }
    if FAKES
        .lock()
        .map(|fakes| fakes.contains_key(name))
        .unwrap_or(false)
    {
        return Err(format!("fake player {name} already exists"));
    }
    // Refuse to shadow a real online player.
    for online in server.worlds.load().iter() {
        if online
            .players
            .load()
            .iter()
            .any(|p| p.gameprofile.name.eq_ignore_ascii_case(name))
        {
            return Err(format!("a real player named {name} is online"));
        }
    }

    let player = spawn_untracked(server, world, name, position, yaw, pitch).await?;
    FAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(name.to_string(), player);
    Ok(())
}

async fn spawn_untracked(
    server: &Arc<Server>,
    world: &Arc<crate::world::World>,
    name: &str,
    position: Vector3<f64>,
    yaw: f32,
    pitch: f32,
) -> Result<Arc<Player>, String> {
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
pub async fn kill(server: &Arc<Server>, name: &str) -> Result<(), String> {
    let player = {
        let mut fakes = FAKES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        fakes
            .remove(name)
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

/// Turns a fake player in place.
pub async fn look_up(player: &Arc<Player>, yaw: f32, pitch: f32) {
    let entity = &player.living_entity.entity;
    entity.yaw.store(yaw);
    entity.head_yaw.store(yaw);
    entity.body_yaw.store(yaw);
    entity.pitch.store(pitch);
    entity.send_pos_rot().await;
}
