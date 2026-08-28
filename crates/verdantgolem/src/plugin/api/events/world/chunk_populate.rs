use verdantgolem_macros::{Event, cancellable};
use verdantgolem_util::math::vector2::Vector2;

/// An event that occurs when a chunk is populated / generated.
#[cancellable]
#[derive(Event, Clone)]
pub struct ChunkPopulateEvent {
    /// Chunk coordinates.
    pub chunk_pos: Vector2<i32>,
}

impl ChunkPopulateEvent {
    #[must_use]
    pub const fn new(chunk_pos: Vector2<i32>) -> Self {
        Self {
            chunk_pos,
            cancelled: false,
        }
    }
}
