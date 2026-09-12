//! Carpet-style fake players (`/player`), backed by a headless
//! [`ClientPlatform::Local`] connection.
//!
//! They load chunks, appear in the tab list and to other players, keep farms
//! running while nobody is online, but have no network connection and never
//! receive packets.

pub mod actions;

use self::actions::{ActionKind, ActionMode, ActionScheduler, MovementDirection};

use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock, Mutex, atomic::Ordering},
};

use uuid::Uuid;

use crate::{
    entity::{EntityBase, player::Player},
    net::{ClientPlatform, GameProfile},
    server::Server,
};
use arc_swap::ArcSwap;
use verdantgolem_data::entity::EntityType;
use verdantgolem_data::tag::Taggable;
use verdantgolem_util::{
    Hand,
    math::{boundingbox::BoundingBox, vector3::Vector3},
};

/// A fake player plus its repeating-action state.
pub struct FakePlayer {
    player: Arc<Player>,
    actions: Mutex<ActionScheduler>,
}

enum FakeEntry {
    Spawning,
    Online(Arc<FakePlayer>),
}

/// Fake-player names are reserved before the first await so concurrent spawn
/// commands cannot create duplicates. Keys use vanilla's case-insensitive
/// player-name semantics.
static FAKES: LazyLock<Mutex<BTreeMap<String, FakeEntry>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

struct SpawnReservation {
    key: String,
    active: bool,
}

impl SpawnReservation {
    fn reserve(name: &str) -> Result<Self, String> {
        let key = normalize_name(name);
        let mut fakes = FAKES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if fakes.contains_key(&key) {
            return Err(format!("fake player {name} already exists"));
        }
        fakes.insert(key.clone(), FakeEntry::Spawning);
        Ok(Self { key, active: true })
    }

    fn commit(mut self, fake: Arc<FakePlayer>) {
        FAKES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(self.key.clone(), FakeEntry::Online(fake));
        self.active = false;
    }
}

impl Drop for SpawnReservation {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut fakes = FAKES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if matches!(fakes.get(&self.key), Some(FakeEntry::Spawning)) {
            fakes.remove(&self.key);
        }
    }
}

/// Vanilla-compatible offline UUID for a fake player name (stable across restarts).
#[must_use]
pub fn offline_uuid(name: &str) -> Uuid {
    let mut bytes = *md5::compute(format!("OfflinePlayer:{name}").as_bytes());
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

/// Whether `name` is a legal Minecraft player name.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    (3..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn normalize_name(name: &str) -> String {
    name.to_ascii_lowercase()
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
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&normalize_name(name))
        .and_then(|entry| match entry {
            FakeEntry::Online(fake) => Some(fake.player.clone()),
            FakeEntry::Spawning => None,
        })
}

fn get_fake(name: &str) -> Result<Arc<FakePlayer>, String> {
    FAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&normalize_name(name))
        .and_then(|entry| match entry {
            FakeEntry::Online(fake) => Some(fake.clone()),
            FakeEntry::Spawning => None,
        })
        .ok_or_else(|| format!("no fake player named {name}"))
}

/// Whether a fake player has an attack action scheduled.
#[must_use]
pub fn is_attacking(name: &str) -> bool {
    get_fake(name).is_ok_and(|fake| {
        fake.actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_active(ActionKind::Attack)
    })
}

/// Preserves the original bare `/player <name> attack` toggle.
pub fn set_attacking(name: &str, on: bool) -> Result<(), String> {
    set_action(
        name,
        ActionKind::Attack,
        on.then_some(ActionMode::Continuous),
    )
}

/// Replaces one action without changing the other action or movement.
pub fn set_action(name: &str, kind: ActionKind, mode: Option<ActionMode>) -> Result<(), String> {
    let fake = get_fake(name)?;
    fake.actions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .set(kind, mode);
    // Replacing use releases any previously held item, including a charged bow.
    // Never hold the scheduler/registry lock across callbacks into plugins.
    if kind == ActionKind::Use {
        fake.player.release_item_use();
    }
    Ok(())
}

/// Sets continuous local-space movement, applied by normal collision physics.
pub fn set_movement(name: &str, direction: Option<MovementDirection>) -> Result<(), String> {
    let fake = get_fake(name)?;
    fake.actions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .movement = direction;
    if direction.is_none() {
        fake.player.clear_local_movement();
    }
    Ok(())
}

/// Clears all actions and releases held use, jump, sneak, and movement input.
pub fn stop_actions(name: &str) -> Result<(), String> {
    let fake = get_fake(name)?;
    fake.actions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .stop();
    fake.player.release_item_use();
    fake.player.clear_local_movement();
    let entity = &fake.player.living_entity.entity;
    entity.set_sneaking(false);
    fake.player.set_sprinting(false);
    fake.player.stop_mining();
    Ok(())
}

/// Called once per simulated world tick, before player physics. Network-only
/// frozen ticks neither consume scheduled actions nor advance their delays.
pub fn tick_fakes(world: &Arc<crate::world::World>) {
    let fakes: Vec<Arc<FakePlayer>> = FAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .values()
        .filter_map(|entry| match entry {
            FakeEntry::Online(fake) if Arc::ptr_eq(&fake.player.world(), world) => {
                Some(fake.clone())
            }
            FakeEntry::Spawning | FakeEntry::Online(_) => None,
        })
        .collect();
    for fake in fakes {
        let player = &fake.player;
        if !player.can_perform_actions() {
            player.clear_local_movement();
            player.cancel_item_use();
            player.stop_mining();
            continue;
        }
        let actions = fake
            .actions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tick();
        if actions.release_use {
            player.release_item_use();
        }
        let input = actions
            .movement
            .map_or((0.0, 0.0), MovementDirection::input);
        player.set_local_movement(input.0, input.1);
        // A held use takes priority over attack, just as with a real client's
        // right mouse button. Consumed/cancelled use never falls through.
        let used = actions.use_item && player.use_look_target();
        if actions.attack && !used && !player.is_using_item() {
            try_attack(player);
        }
    }
}

/// Attack the crosshair target through the shared real-player interaction path.
fn try_attack(player: &Arc<Player>) {
    use crate::entity::player::interaction::LookTarget;
    use verdantgolem_protocol::java::server::play::ActionType;

    if let LookTarget::Entity(target, _) = player.look_target()
        && let Some(server) = player.world().server.upgrade()
    {
        player.interact_entity(target, ActionType::Attack, Hand::Right, None, &server);
        player.swing_hand(Hand::Right, true);
    }
}

pub fn list() -> Vec<String> {
    FAKES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .values()
        .filter_map(|entry| match entry {
            FakeEntry::Online(fake) => Some(fake.player.gameprofile.name.clone()),
            FakeEntry::Spawning => None,
        })
        .collect()
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
    let reservation = SpawnReservation::reserve(name)?;
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
    reservation.commit(Arc::new(FakePlayer {
        player,
        actions: Mutex::new(ActionScheduler::default()),
    }));
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
    let Some((player, spawn_world)) =
        server.add_player(Arc::new(ClientPlatform::Local), profile(name), None)
    else {
        return Err(format!("failed to create fake player {name}"));
    };

    let entity = &player.living_entity.entity;

    // add_player defaults to the first world; move the fake into the
    // sender's world when they differ, including chunk-manager ownership.
    if !Arc::ptr_eq(&spawn_world, world) {
        let Some(removed) = spawn_world.remove_player(&player, false).await else {
            server.remove_player(&player);
            return Err(format!("failed to move fake player {name} between worlds"));
        };
        world.add_player(&removed)?;
        player.unload_watched_chunks(&spawn_world).await;
        player
            .chunk_manager
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .change_world(&spawn_world.level, world.clone());
        entity.set_world(world.clone());
    }

    // Set every spatial cache before chunk tracking and the spawn broadcast.
    entity.set_pos(position);
    entity.set_rotation(yaw, pitch);
    entity.head_yaw.store(yaw);
    entity.body_yaw.store(yaw);
    entity.last_pos.store(position);
    entity.last_sent_pos.store(position);
    crate::world::chunker::update_position(&player);
    world
        .level
        .get_or_fetch_chunk(entity.chunk_pos.load(), |_| ())
        .await;
    // A previously killed fake player was saved with 0 health; revive it so
    // the respawned entity isn't a permanent corpse (carpet parity).
    if player.living_entity.health.load() <= 0.0 {
        player.living_entity.reset_state();
    }

    world.spawn_local_player(&player);

    Ok(player)
}

/// Removes a fake player by name.
pub async fn kill(server: &Server, name: &str) -> Result<(), String> {
    let player = {
        let mut fakes = FAKES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let key = normalize_name(name);
        if matches!(fakes.get(&key), Some(FakeEntry::Spawning)) {
            return Err(format!("fake player {name} is still spawning"));
        }
        match fakes.remove(&key) {
            Some(FakeEntry::Online(fake)) => {
                fake.actions
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .stop();
                // Outstanding tick snapshots may still hold this Arc. Mark
                // it removed before the first await so every action/physics
                // entry point rejects it during asynchronous chunk cleanup.
                fake.player
                    .get_entity()
                    .removed
                    .store(true, Ordering::Relaxed);
                fake.player.clone()
            }
            Some(FakeEntry::Spawning) => {
                fakes.insert(key, FakeEntry::Spawning);
                return Err(format!("fake player {name} is still spawning"));
            }
            None => return Err(format!("no fake player named {name}")),
        }
    };

    player.cancel_item_use();
    player.clear_local_movement();
    player.stop_mining();
    player.remove().await;
    server.remove_player(&player);
    let player_data_error = server
        .player_data_storage
        .handle_player_leave(&player)
        .err();
    let advancement_error = server.advancement_manager.save_player(&player).await.err();
    match (player_data_error, advancement_error) {
        (None, None) => Ok(()),
        (Some(data), None) => Err(format!("failed to save fake player data: {data}")),
        (None, Some(advancement)) => Err(format!(
            "failed to save fake player advancements: {advancement}"
        )),
        (Some(data), Some(advancement)) => Err(format!(
            "failed to save fake player data: {data}; failed to save advancements: {advancement}"
        )),
    }
}

fn mount_capacity(entity: &Arc<dyn EntityBase>) -> Option<usize> {
    let entity_type = entity.get_entity().entity_type;
    if entity_type == &EntityType::PLAYER {
        return None;
    }
    if entity.get_living_entity().is_some() {
        return Some(if entity_type == &EntityType::HAPPY_GHAST {
            4
        } else if entity_type == &EntityType::CAMEL || entity_type == &EntityType::CAMEL_HUSK {
            2
        } else {
            1
        });
    }
    if entity
        .cast_any()
        .is::<crate::entity::vehicle::boat::BoatEntity>()
    {
        return Some(2);
    }
    (entity_type == &EntityType::MINECART).then_some(1)
}

/// Mounts a fake player onto the nearest mountable entity (boats, minecarts,
/// mobs...). Returns a description of the vehicle.
pub fn mount(name: &str) -> Result<String, String> {
    let player = get(name).ok_or_else(|| format!("no fake player named {name}"))?;
    let entity = &player.living_entity.entity;
    if entity.has_vehicle() {
        return Err(format!("{name} is already riding an entity"));
    }
    if entity.riding_cooldown.load(Ordering::Relaxed) > 0 {
        return Err(format!("{name} cannot mount during its riding cooldown"));
    }
    let pos = entity.pos.load();
    let search_box = BoundingBox::new(pos.sub_raw(3.0, 3.0, 3.0), pos.add_raw(3.0, 3.0, 3.0));
    let world = entity.world.load();

    let mut best: Option<(f64, Arc<dyn crate::entity::EntityBase>)> = None;
    for other in world.get_entities_at_box(&search_box) {
        let other_entity = other.get_entity();
        if other_entity.entity_id == entity.entity_id || !other_entity.is_alive() {
            continue;
        }
        let Some(capacity) = mount_capacity(&other) else {
            continue;
        };
        if other_entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
            >= capacity
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
    let capacity = mount_capacity(&vehicle).ok_or_else(|| {
        format!(
            "{} is no longer mountable",
            vehicle.get_entity().entity_type.registry_key()
        )
    })?;
    if vehicle
        .get_entity()
        .passengers
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .len()
        >= capacity
    {
        return Err(format!(
            "{} has no free passenger seat",
            vehicle.get_entity().entity_type.registry_key()
        ));
    }
    let vehicle_name = vehicle.get_entity().entity_type.registry_key();
    let player_base: Arc<dyn crate::entity::EntityBase> = player.clone();
    vehicle
        .get_entity()
        .add_passenger(vehicle.clone(), player_base);

    let mounted = entity
        .vehicle
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .is_some_and(|mounted| mounted.get_entity().entity_id == vehicle.get_entity().entity_id);
    if !mounted {
        return Err(format!("mounting {vehicle_name} was cancelled"));
    }

    Ok(vehicle_name.to_string())
}

/// Dismounts a fake player from its vehicle.
pub fn dismount(name: &str) -> Result<(), String> {
    let player = get(name).ok_or_else(|| format!("no fake player named {name}"))?;
    let entity = &player.living_entity.entity;
    let vehicle = entity
        .vehicle
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let Some(vehicle) = vehicle else {
        return Err(format!("{name} is not riding anything"));
    };
    vehicle.get_entity().remove_passenger(entity.entity_id);
    if entity
        .vehicle
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some()
    {
        return Err(format!("dismounting {name} was cancelled"));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_vanilla_known_vector() {
        assert_eq!(
            offline_uuid("Steve"),
            Uuid::parse_str("5627dd98-e6be-3c21-b8a8-e92344183641").unwrap()
        );
    }

    #[test]
    fn name_validation_matches_vanilla_boundaries() {
        assert!(valid_name("abc"));
        assert!(valid_name("abcdefghijklmnop"));
        assert!(valid_name("farm_bot_1"));
        assert!(!valid_name("ab"));
        assert!(!valid_name("abcdefghijklmnopq"));
        assert!(!valid_name("fake-player"));
        assert!(!valid_name("bad name"));
        assert!(!valid_name("玩家abc"));
    }

    #[test]
    fn reservation_is_case_insensitive_and_released_on_drop() {
        let first = SpawnReservation::reserve("ReserveBot").unwrap();
        assert!(SpawnReservation::reserve("reservebot").is_err());
        drop(first);

        let second = SpawnReservation::reserve("RESERVEBOT").unwrap();
        drop(second);
    }
}
