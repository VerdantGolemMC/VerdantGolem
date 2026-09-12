use std::any::Any;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_util::Hand;

use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata, consume_item_in_hand};
use verdantgolem_data::item::Item;
use verdantgolem_data::sound::{Sound, SoundCategory};

pub struct ExperienceBottleItem;

impl ItemMetadata for ExperienceBottleItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::EXPERIENCE_BOTTLE.id])
    }
}

impl ItemBehaviour for ExperienceBottleItem {
    fn normal_use(&self, _stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let world = player.world();
        let pos = player.eye_position();
        world.play_sound(
            Sound::EntityExperienceBottleThrow,
            SoundCategory::Players,
            &pos,
        );

        let amount = (rand::random::<u32>() % 9 + 3) as u32; // 3..=11 exp
        ExperienceOrbEntity::spawn(&world, pos, amount);

        consume_item_in_hand(player, hand, 1);
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
