use std::any::Any;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_util::Hand;

use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use verdantgolem_data::item::Item;
use verdantgolem_data::sound::{Sound, SoundCategory};

pub struct SpyglassItem;

impl ItemMetadata for SpyglassItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::SPYGLASS.id])
    }
}

impl ItemBehaviour for SpyglassItem {
    fn normal_use(&self, stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        player.world().play_sound(
            Sound::ItemSpyglassUse,
            SoundCategory::Players,
            &player.position(),
        );
        player
            .living_entity
            .set_active_hand(hand, stack.clone(), Self::USE_DURATION);
        true
    }

    fn on_stopped_using(&self, _stack: &ItemStack, player: &Player, _hand: Hand) {
        player.world().play_sound(
            Sound::ItemSpyglassStopUsing,
            SoundCategory::Players,
            &player.position(),
        );
    }

    fn get_use_duration(&self) -> i32 {
        Self::USE_DURATION
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl SpyglassItem {
    pub const USE_DURATION: i32 = 1200;
}
