use crate::entity::player::Player;
use std::sync::Arc;
use verdantgolem_macros::{Event, cancellable};
use verdantgolem_util::math::position::BlockPos;

/// An event that occurs when a sign's text is changed.
#[cancellable]
#[derive(Event, Clone)]
pub struct SignChangeEvent {
    pub player: Arc<Player>,
    pub block_pos: BlockPos,
    pub lines: Vec<String>,
}

impl SignChangeEvent {
    #[must_use]
    pub const fn new(player: Arc<Player>, block_pos: BlockPos, lines: Vec<String>) -> Self {
        Self {
            player,
            block_pos,
            lines,
            cancelled: false,
        }
    }
}
