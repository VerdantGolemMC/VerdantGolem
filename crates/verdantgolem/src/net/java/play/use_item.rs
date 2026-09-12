#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_use_item(&self, player: &Arc<Player>, use_item: &SUseItem, server: &Arc<Server>) {
        if !player.has_client_loaded() {
            return;
        }
        let Ok(hand) = Hand::from_packet_id(use_item.hand.0) else {
            self.try_kick(&TextComponent::text("InvalidHand"));
            return;
        };
        self.update_sequence(use_item.sequence.0);
        player.use_item(hand, use_item.yaw, use_item.pitch, server);
    }
}
