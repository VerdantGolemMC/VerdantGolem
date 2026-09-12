use rand::{RngExt, rng};
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_util::Hand;

use std::sync::Arc;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::ender_pearl::EnderPearlEntity;
use crate::item::{ItemBehaviour, ItemMetadata, consume_item_in_hand};
use verdantgolem_data::entity::EntityType;
use verdantgolem_data::item::Item;
use verdantgolem_data::sound::Sound;

pub struct EnderPearlItem;

impl ItemMetadata for EnderPearlItem {
    fn ids() -> Box<[u16]> {
        [Item::ENDER_PEARL.id].into()
    }
}

const ROLL: f32 = 0.0;
const POWER: f32 = 1.5;
const DIVERGENCE: f32 = 1.0;
const THROW_SOUND_VOLUME: f32 = 0.5;

impl ItemBehaviour for EnderPearlItem {
    fn normal_use(&self, _stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let position = player.position();
        let world = player.world();
        world.play_sound_fine(
            Sound::EntityEnderPearlThrow,
            verdantgolem_data::sound::SoundCategory::Neutral,
            &position,
            THROW_SOUND_VOLUME,
            0.4 / (rng().random::<f32>() * 0.4 + 0.8),
        );

        let entity = Entity::new(world.clone(), position, &EntityType::ENDER_PEARL);
        let pearl = EnderPearlEntity::new_shot(entity, player.get_entity());
        let (yaw, pitch) = player.rotation();
        pearl
            .thrown
            .set_velocity_from(pitch, yaw, ROLL, POWER, DIVERGENCE);
        world.spawn_entity(Arc::new(pearl));

        consume_item_in_hand(player, hand, 1);
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
