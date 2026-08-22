use super::PlayerEvent;
use crate::entity::player::Player;
use std::sync::Arc;
use verdantgolem_macros::{Event, cancellable};
use verdantgolem_util::math::vector3::Vector3;

/// An event that occurs when determining player spawn position.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerSpawnLocationEvent {
    /// The player.
    pub player: Arc<Player>,
    /// Spawn position.
    pub spawn_pos: Vector3<f64>,
}

impl PlayerSpawnLocationEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, spawn_pos: Vector3<f64>) -> Self {
        Self {
            player,
            spawn_pos,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerSpawnLocationEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
