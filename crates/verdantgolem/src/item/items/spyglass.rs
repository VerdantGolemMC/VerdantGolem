use std::any::Any;

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
    fn normal_use(&self, _item: &Item, player: &Player) {
        player.world().play_sound(
            Sound::ItemSpyglassUse,
            SoundCategory::Players,
            &player.position(),
        );
        let stack = player.inventory().held_item();
        player
            .living_entity
            .set_active_hand(verdantgolem_util::Hand::Right, stack, Self::USE_DURATION);
    }

    fn on_stopped_using(&self, _stack: &verdantgolem_data::item_stack::ItemStack, player: &Player) {
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
