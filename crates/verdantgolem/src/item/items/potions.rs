use std::sync::Arc;
use verdantgolem_util::Hand;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::{
    lingering_potion::LingeringPotionEntity, splash_potion::SplashPotionEntity,
};
use crate::item::{ItemBehaviour, ItemMetadata, consume_item_in_hand};
use verdantgolem_data::entity::EntityType;
use verdantgolem_data::item::Item;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_data::sound::Sound;

pub struct PotionItem;
pub struct SplashPotionItem;
pub struct LingeringPotionItem;

impl ItemMetadata for PotionItem {
    fn ids() -> Box<[u16]> {
        [Item::POTION.id].into()
    }
}

impl ItemMetadata for SplashPotionItem {
    fn ids() -> Box<[u16]> {
        [Item::SPLASH_POTION.id].into()
    }
}

impl ItemMetadata for LingeringPotionItem {
    fn ids() -> Box<[u16]> {
        [Item::LINGERING_POTION.id].into()
    }
}

const POWER: f32 = 0.5;

impl ItemBehaviour for PotionItem {
    fn normal_use(&self, _stack: &ItemStack, _player: &Player, _hand: Hand) -> bool {
        // Drinking is handled by the consumable flow in the server (active hand + consumption tick).
        false
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ItemBehaviour for SplashPotionItem {
    fn normal_use(&self, stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let position = player.position();
        let world = player.world();
        world.play_sound(
            Sound::EntityWitchThrow,
            verdantgolem_data::sound::SoundCategory::Neutral,
            &position,
        );
        let entity = Entity::new(world.clone(), position, &EntityType::SPLASH_POTION);
        let splash = SplashPotionEntity::new_shot(entity, player.get_entity());

        // Keep the selected hand's potion components, not a same-ID stack in the other hand.
        splash.set_item_stack(stack.copy_with_count(1));

        let (yaw, pitch) = player.rotation();
        splash.thrown.set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);

        world.spawn_entity(Arc::new(splash));

        consume_item_in_hand(player, hand, 1);
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ItemBehaviour for LingeringPotionItem {
    fn normal_use(&self, stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let position = player.position();
        let world = player.world();
        world.play_sound(
            Sound::EntityWitchThrow,
            verdantgolem_data::sound::SoundCategory::Neutral,
            &position,
        );
        let entity = Entity::new(world.clone(), position, &EntityType::LINGERING_POTION);
        let ling = LingeringPotionEntity::new_shot(entity, player.get_entity());

        // Keep the selected hand's potion components, not a same-ID stack in the other hand.
        ling.set_item_stack(stack.copy_with_count(1));

        let (yaw, pitch) = player.rotation();
        ling.thrown.set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);

        world.spawn_entity(Arc::new(ling));

        consume_item_in_hand(player, hand, 1);
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
