#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_use_item_on(
        &self,
        player: &Arc<Player>,
        use_item_on: &SUseItemOn,
        server: &Arc<Server>,
    ) -> Result<(), BlockPlacingError> {
        if !player.has_client_loaded() {
            return Ok(());
        }
        self.update_sequence(use_item_on.sequence.0);
        let face = BlockDirection::try_from(use_item_on.face.0)
            .map_err(|_| BlockPlacingError::InvalidBlockFace)?;
        let hand =
            Hand::from_packet_id(use_item_on.hand.0).map_err(|_| BlockPlacingError::InvalidHand)?;
        let hit = crate::block::BlockUseContext {
            position: use_item_on.position,
            face,
            cursor_pos: use_item_on.cursor_pos,
        };
        player.use_item_on(&hit, hand, server).map(|_| ())
    }
}
