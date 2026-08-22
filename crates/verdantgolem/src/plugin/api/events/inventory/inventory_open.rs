use crate::entity::player::Player;
use std::sync::Arc;
use verdantgolem_macros::{Event, cancellable};

/// An event that occurs when a player opens an inventory.
#[cancellable]
#[derive(Event, Clone)]
pub struct InventoryOpenEvent {
    /// The player opening the inventory.
    pub player: Arc<Player>,
}

impl InventoryOpenEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>) -> Self {
        Self {
            player,
            cancelled: false,
        }
    }
}
