use crate::entity::player::Player;
use std::sync::Arc;
use verdantgolem_macros::Event;

/// An event that occurs when items are placed into a grindstone.
#[derive(Event, Clone)]
pub struct PrepareGrindstoneEvent {
    /// The player using the grindstone.
    pub player: Arc<Player>,

    /// The resulting item ID prepared in the output slot.
    pub result_item: Option<String>,
}

impl PrepareGrindstoneEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, result_item: Option<String>) -> Self {
        Self {
            player,
            result_item,
        }
    }
}
