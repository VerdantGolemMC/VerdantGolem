use std::sync::Arc;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_macros::{Event, cancellable};
use verdantgolem_util::math::position::BlockPos;

use super::PlayerEvent;
use crate::entity::player::Player;

/// An event that occurs when a player harvests a block (e.g. sweet berry bush, bee hive).
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerHarvestBlockEvent {
    /// The player harvesting the block.
    pub player: Arc<Player>,

    /// The position of the harvested block.
    pub block_pos: BlockPos,

    /// The items harvested.
    pub harvested_items: Vec<ItemStack>,
}

impl PlayerEvent for PlayerHarvestBlockEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
