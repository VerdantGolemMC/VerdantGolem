use std::any::Any;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_util::Hand;

use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use verdantgolem_data::item::Item;
use verdantgolem_data::sound::{Sound, SoundCategory};

pub struct ClockItem;

impl ItemMetadata for ClockItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::CLOCK.id])
    }
}

impl ItemBehaviour for ClockItem {
    fn normal_use(&self, _stack: &ItemStack, player: &Player, _hand: Hand) -> bool {
        let world = player.world();
        world.play_sound(
            Sound::UiButtonClick,
            SoundCategory::Players,
            &player.position(),
        );
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
