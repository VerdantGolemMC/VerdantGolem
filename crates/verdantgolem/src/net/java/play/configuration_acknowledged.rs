#[allow(clippy::wildcard_imports)]
use super::*;
use verdantgolem_protocol::ConnectionState;

impl JavaClient {
    pub fn handle_configuration_acknowledged(&self, player: &Player) {
        debug!(
            "Player {} acknowledged configuration switch",
            player.gameprofile.name
        );
        self.connection_state.store(ConnectionState::Config);
    }
}
