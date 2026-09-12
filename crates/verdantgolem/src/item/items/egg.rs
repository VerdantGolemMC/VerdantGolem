use std::sync::Arc;
use verdantgolem_util::Hand;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::egg::EggEntity;
use crate::item::{ItemBehaviour, ItemMetadata, consume_item_in_hand};
use verdantgolem_data::entity::EntityType;
use verdantgolem_data::item::Item;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_data::sound::Sound;

pub struct EggItem;

impl ItemMetadata for EggItem {
    fn ids() -> Box<[u16]> {
        [Item::EGG.id, Item::BLUE_EGG.id, Item::BROWN_EGG.id].into()
    }
}

const POWER: f32 = 1.5;

impl ItemBehaviour for EggItem {
    fn normal_use(&self, stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let position = player.position();
        let world = player.world();
        world.play_sound(
            Sound::EntityEggThrow,
            verdantgolem_data::sound::SoundCategory::Players,
            &position,
        );

        let entity = Entity::new(world.clone(), position, &EntityType::EGG);
        let egg = EggEntity::new_shot(entity, player.get_entity());

        // Propagate the item stack so clients show correct variant
        egg.set_item_stack(stack.copy_with_count(1));

        let (yaw, pitch) = player.rotation();
        egg.thrown.set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);
        world.spawn_entity(Arc::new(egg));

        consume_item_in_hand(player, hand, 1);
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
