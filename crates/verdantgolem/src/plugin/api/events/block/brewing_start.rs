use std::sync::Arc;
use verdantgolem_macros::{Event, cancellable};
use verdantgolem_util::math::position::BlockPos;

use crate::world::World;

/// An event that occurs when a brewing stand starts brewing.
#[cancellable]
#[derive(Event, Clone)]
pub struct BrewingStartEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub brewing_time: i32,
}

impl BrewingStartEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, brewing_time: i32) -> Self {
        Self {
            block_pos,
            world,
            brewing_time,
            cancelled: false,
        }
    }
}
