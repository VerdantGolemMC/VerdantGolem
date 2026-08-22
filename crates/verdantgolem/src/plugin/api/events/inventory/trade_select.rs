use crate::entity::player::Player;
use std::sync::Arc;
use verdantgolem_macros::{Event, cancellable};

/// An event that occurs when a player selects a trade recipe with a merchant.
#[cancellable]
#[derive(Event, Clone)]
pub struct TradeSelectEvent {
    /// The player selecting the trade.
    pub player: Arc<Player>,

    /// The index of the selected trade slot.
    pub slot_index: u8,
}

impl TradeSelectEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, slot_index: u8) -> Self {
        Self {
            player,
            slot_index,
            cancelled: false,
        }
    }
}
