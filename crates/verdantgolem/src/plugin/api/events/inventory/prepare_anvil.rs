use crate::entity::player::Player;
use std::sync::Arc;
use verdantgolem_macros::Event;

/// An event that occurs when an item is prepared in an anvil.
#[derive(Event, Clone)]
pub struct PrepareAnvilEvent {
    /// The player using the anvil.
    pub player: Arc<Player>,

    /// The name prepared in the anvil input text field.
    pub rename_text: String,

    /// The repair cost in experience levels.
    pub repair_cost: u32,
}

impl PrepareAnvilEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, rename_text: String, repair_cost: u32) -> Self {
        Self {
            player,
            rename_text,
            repair_cost,
        }
    }
}
