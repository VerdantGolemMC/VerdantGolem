use crate::world::World;
use std::sync::Arc;
use tokio::sync::RwLock;
use verdantgolem_macros::{Event, cancellable};
use verdantgolem_world::chunk::ChunkData;

/// An event that occurs when a chunk is saved in a world.
///
/// This event contains information about the world and the chunk being saved.
#[cancellable]
#[derive(Event, Clone)]
pub struct ChunkSave {
    /// The world in which the chunk is being saved.
    pub world: Arc<World>,

    /// The chunk data being saved, wrapped in a read-write lock for safe concurrent access.
    pub chunk: Arc<RwLock<ChunkData>>,
}
