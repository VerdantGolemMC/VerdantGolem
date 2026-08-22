use crate::entity::player::Player;
use std::sync::Arc;
use verdantgolem_macros::{Event, cancellable};

/// An event that occurs when a player swaps items between main hand and offhand.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerSwapHandItemsEvent {
    /// The player swapping hand items.
    pub player: Arc<Player>,
}

impl PlayerSwapHandItemsEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>) -> Self {
        Self {
            player,
            cancelled: false,
        }
    }
}
