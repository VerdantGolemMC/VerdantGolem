use crate::world::World;
use std::sync::Arc;
use verdantgolem_macros::{Event, cancellable};
use verdantgolem_world::chunk::ChunkData;

/// An event that occurs when a chunk is sent to a client.
///
/// This event contains information about the world and the chunk being sent.
#[cancellable]
#[derive(Event, Clone)]
pub struct ChunkSend {
    /// The world from which the chunk is being sent.
    pub world: Arc<World>,

    /// The chunk data being sent.
    pub chunk: Arc<ChunkData>,
}

impl ChunkSend {
    pub const fn new(world: Arc<World>, chunk: Arc<ChunkData>) -> Self {
        Self {
            world,
            chunk,
            cancelled: false,
        }
    }
}
