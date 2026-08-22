use super::PlayerEvent;
use crate::entity::player::Player;
use std::sync::Arc;
use verdantgolem_macros::{Event, cancellable};

/// An event that occurs when a player discovers a recipe.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerRecipeDiscoverEvent {
    /// The player.
    pub player: Arc<Player>,
    /// Recipe ID.
    pub recipe_id: String,
}

impl PlayerRecipeDiscoverEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, recipe_id: String) -> Self {
        Self {
            player,
            recipe_id,
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerRecipeDiscoverEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
