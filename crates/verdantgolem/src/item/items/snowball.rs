use std::sync::Arc;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_util::Hand;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::snowball::SnowballEntity;
use crate::item::{ItemBehaviour, ItemMetadata, consume_item_in_hand};
use verdantgolem_data::entity::EntityType;
use verdantgolem_data::item::Item;
use verdantgolem_data::sound::Sound;

pub struct SnowBallItem;

impl ItemMetadata for SnowBallItem {
    fn ids() -> Box<[u16]> {
        [Item::SNOWBALL.id].into()
    }
}

const POWER: f32 = 1.5;

impl ItemBehaviour for SnowBallItem {
    fn normal_use(&self, _stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let position = player.position();
        let world = player.world();
        world.play_sound(
            Sound::EntitySnowballThrow,
            verdantgolem_data::sound::SoundCategory::Neutral,
            &position,
        );
        let entity = Entity::new(world.clone(), position, &EntityType::SNOWBALL);
        let snowball = SnowballEntity::new_shot(entity, player.get_entity());
        let (yaw, pitch) = player.rotation();
        snowball
            .thrown
            .set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);
        world.spawn_entity(Arc::new(snowball));

        consume_item_in_hand(player, hand, 1);
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
