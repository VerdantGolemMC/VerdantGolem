use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_macros::{Event, cancellable};
use verdantgolem_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// An event that occurs when an automated crafter crafts an item.
#[cancellable]
#[derive(Event, Clone)]
pub struct CrafterCraftEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub result: ItemStack,
}

impl CrafterCraftEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, result: ItemStack) -> Self {
        Self {
            block_pos,
            world,
            result,
            cancelled: false,
        }
    }
}
