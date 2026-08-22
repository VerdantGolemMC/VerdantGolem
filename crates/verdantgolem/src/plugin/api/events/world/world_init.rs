use crate::world::World;
use std::sync::Arc;
use verdantgolem_macros::Event;

/// An event that occurs when a world is initialized.
#[derive(Event, Clone)]
pub struct WorldInitEvent {
    /// The world being initialized.
    pub world: Arc<World>,
}

impl WorldInitEvent {
    #[must_use]
    pub const fn new(world: Arc<World>) -> Self {
        Self { world }
    }
}
