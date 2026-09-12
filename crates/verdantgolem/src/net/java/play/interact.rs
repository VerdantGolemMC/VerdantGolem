#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_interact(
        &self,
        player: &Arc<Player>,
        interact: &SInteract,
        server: &Arc<Server>,
    ) {
        if !player.has_client_loaded() {
            return;
        }
        player.update_last_action_time();
        let entity_id = interact.entity_id;
        let player_entity = player.get_entity();
        if player_entity.is_sneaking() != interact.sneaking {
            player_entity.set_sneaking(interact.sneaking);
        }
        let Ok(action) = ActionType::try_from(interact.r#type.0) else {
            self.try_kick(&TextComponent::text("Invalid action type"));
            return;
        };
        let Ok(hand) = Hand::from_packet_id(interact.hand.map_or(0, |hand| hand.0)) else {
            self.try_kick(&TextComponent::text("InvalidHand"));
            return;
        };
        let world = player.world();
        let target = world
            .get_player_by_id(entity_id.0)
            .map(|target| target as Arc<dyn EntityBase>)
            .or_else(|| world.get_entity_by_id(entity_id.0));
        if let Some(target) = target {
            if player.gamemode.load() == GameMode::Spectator {
                player.camera_target_id.store(Some(entity_id.0));
                player.try_send_client_packet(&CSetCamera::new(entity_id));
                return;
            }
            if action == ActionType::Attack && entity_id.0 == player.entity_id() {
                self.try_kick(&TextComponent::translate_cross(
                    translation::java::MULTIPLAYER_DISCONNECT_INVALID_ENTITY_ATTACKED,
                    translation::java::MULTIPLAYER_DISCONNECT_INVALID_ENTITY_ATTACKED,
                    [],
                ));
                return;
            }
            player.interact_entity(target, action, hand, interact.target_position, server);
        } else {
            send_cancellable_blocking! {{
                server;
                PlayerInteractUnknownEntityEvent::new(player, entity_id.0, action);
                'after: {
                    if event.action == ActionType::Attack {
                        error!(
                            "Player id {} interacted with entity id {}, which was not found.",
                            player.entity_id(),
                            event.entity_id
                        );
                        self.try_kick(&TextComponent::translate_cross(
                            translation::java::MULTIPLAYER_DISCONNECT_INVALID_ENTITY_ATTACKED,
                            translation::java::MULTIPLAYER_DISCONNECT_INVALID_ENTITY_ATTACKED,
                            [],
                        ));
                    }
                }
            }}
        }
    }
}
