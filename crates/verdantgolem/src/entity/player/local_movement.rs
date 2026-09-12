//! Headless player input and bookkeeping. The actual travel/collision code is
//! shared with living entities; Java and Bedrock retain client-driven motion.

use std::sync::atomic::Ordering;

use verdantgolem_data::attributes::Attributes;
use verdantgolem_protocol::java::server::play::SPlayerInput;
use verdantgolem_util::math::vector3::Vector3;

use crate::{
    entity::EntityBase,
    net::ClientPlatform,
    plugin::{
        api::events::player::player_input::PlayerInputEvent, player::player_move::PlayerMoveEvent,
    },
};

use super::{Player, statistics::StatisticCategory};

struct LocalMovementState {
    local: bool,
    alive: bool,
    spectator: bool,
    riding: bool,
    sleeping: bool,
}

impl LocalMovementState {
    const fn enabled(self) -> bool {
        self.local && self.alive && !self.spectator && !self.riding && !self.sleeping
    }
}

impl Player {
    pub(crate) fn is_local(&self) -> bool {
        matches!(self.client.as_ref(), ClientPlatform::Local)
    }

    pub(crate) fn should_tick_local_physics(&self) -> bool {
        LocalMovementState {
            local: self.is_local(),
            alive: self.is_alive_for_actions(),
            spectator: self.is_spectator(),
            riding: self.get_entity().has_vehicle(),
            sleeping: self.sleeping_since.load().is_some(),
        }
        .enabled()
            && self
                .world()
                .level
                .is_chunk_loaded(&self.get_entity().chunk_pos.load())
    }

    pub(crate) fn clear_local_movement(&self) {
        if !self.is_local() {
            return;
        }
        self.living_entity.movement_input.store(Vector3::default());
        self.living_entity.jumping.store(false, Ordering::Relaxed);
        self.last_input.store(0, Ordering::Relaxed);
    }

    pub(crate) fn set_local_movement(&self, sideways: f64, forward: f64) {
        if !self.should_tick_local_physics() || !sideways.is_finite() || !forward.is_finite() {
            self.clear_local_movement();
            if self.is_local() && self.get_entity().has_vehicle() {
                self.get_entity().velocity.store(Vector3::default());
            }
            return;
        }
        let entity = self.get_entity();
        let mut input = 0;
        if forward > 0.0 {
            input |= SPlayerInput::FORWARD;
        }
        if forward < 0.0 {
            input |= SPlayerInput::BACKWARD;
        }
        if sideways > 0.0 {
            input |= SPlayerInput::LEFT;
        }
        if sideways < 0.0 {
            input |= SPlayerInput::RIGHT;
        }
        let mut factor = if entity.is_sneaking() {
            input |= SPlayerInput::SNEAK;
            self.living_entity
                .get_attribute_value(&Attributes::SNEAKING_SPEED)
        } else {
            1.0
        };
        if entity.is_sprinting() {
            input |= SPlayerInput::SPRINT;
        }
        if self.is_using_item() {
            factor *= 0.2;
        }
        if self.last_input.load(Ordering::Relaxed) != input
            && let Some(server) = self.world().server.upgrade()
            && let Some(player) = self.world().get_player_by_uuid(self.gameprofile.id)
        {
            let mut event = PlayerInputEvent::new(player, format!("{input:b}"));
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                self.clear_local_movement();
                return;
            }
        }
        self.last_input.store(input, Ordering::Relaxed);
        self.living_entity.movement_input.store(Vector3::new(
            sideways.clamp(-1.0, 1.0) * factor,
            0.0,
            forward.clamp(-1.0, 1.0) * factor,
        ));
    }

    /// The same movement plugin callback used for network players. A plugin's
    /// replacement destination remains subject to collision resolution.
    pub(crate) fn local_movement_delta(&self, motion: Vector3<f64>) -> Option<Vector3<f64>> {
        if !self.should_tick_local_physics() {
            return None;
        }
        if motion.length_squared() < 1.0e-12 {
            return Some(motion);
        }
        let world = self.world();
        let server = world.server.upgrade()?;
        let player = world.get_player_by_uuid(self.gameprofile.id)?;
        let position = self.get_entity().pos.load();
        let mut event = PlayerMoveEvent::new(player, position, position + motion);
        server.plugin_manager.fire_blocking(&server, &mut event);
        if event.cancelled
            || !event.to.x.is_finite()
            || !event.to.y.is_finite()
            || !event.to.z.is_finite()
        {
            return None;
        }
        Some(event.to - position)
    }

    /// Keep chunk tickets, movement statistics, exhaustion and observers in
    /// sync after physics (and after a vehicle has repositioned its passenger).
    pub(crate) fn sync_local_movement(&self) {
        if !self.is_local() {
            return;
        }
        let entity = self.get_entity();
        let delta = entity.pos.load() - entity.last_pos.load();
        if delta.length_squared() > 1.0e-5 {
            self.update_last_action_time();
            let distance_cm = (delta.length() * 100.0).round() as i32;
            if distance_cm > 0 {
                self.increment_stat(
                    StatisticCategory::Custom,
                    self.get_movement_statistic() as i32,
                    distance_cm,
                );
            }
            self.progress_motion(delta);
        }
        if let Some(player) = self.world().get_player_by_uuid(self.gameprofile.id) {
            crate::world::chunker::update_position(&player);
        }
        entity.send_pos_rot();
    }
}

#[cfg(test)]
mod tests {
    use super::LocalMovementState;

    const ACTIVE: LocalMovementState = LocalMovementState {
        local: true,
        alive: true,
        spectator: false,
        riding: false,
        sleeping: false,
    };

    #[test]
    fn network_players_never_receive_server_side_travel() {
        assert!(
            !LocalMovementState {
                local: false,
                ..ACTIVE
            }
            .enabled()
        );
        assert!(ACTIVE.enabled());
    }

    #[test]
    fn dead_spectating_riding_and_sleeping_fakes_do_not_travel() {
        assert!(
            !LocalMovementState {
                alive: false,
                ..ACTIVE
            }
            .enabled()
        );
        assert!(
            !LocalMovementState {
                spectator: true,
                ..ACTIVE
            }
            .enabled()
        );
        assert!(
            !LocalMovementState {
                riding: true,
                ..ACTIVE
            }
            .enabled()
        );
        assert!(
            !LocalMovementState {
                sleeping: true,
                ..ACTIVE
            }
            .enabled()
        );
    }
}
