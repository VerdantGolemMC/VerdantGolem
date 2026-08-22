use std::sync::Arc;
use verdantgolem_macros::{Event, cancellable};
use verdantgolem_util::math::position::BlockPos;

use crate::world::World;

/// An event that occurs when farmland moisture level changes.
#[cancellable]
#[derive(Event, Clone)]
pub struct MoistureChangeEvent {
    pub block_pos: BlockPos,
    pub world: Arc<World>,
    pub new_moisture: i32,
}

impl MoistureChangeEvent {
    #[must_use]
    pub const fn new(block_pos: BlockPos, world: Arc<World>, new_moisture: i32) -> Self {
        Self {
            block_pos,
            world,
            new_moisture,
            cancelled: false,
        }
    }
}
