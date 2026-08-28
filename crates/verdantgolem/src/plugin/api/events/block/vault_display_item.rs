use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_macros::{Event, cancellable};
use verdantgolem_util::math::position::BlockPos;
use std::sync::Arc;

use crate::world::World;

/// An event that occurs when a vault displays an item.
#[cancellable]
#[derive(Event, Clone)]
pub struct VaultDisplayItemEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub item: ItemStack,
}

impl VaultDisplayItemEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, item: ItemStack) -> Self {
        Self {
            block_pos,
            world,
            item,
            cancelled: false,
        }
    }
}
