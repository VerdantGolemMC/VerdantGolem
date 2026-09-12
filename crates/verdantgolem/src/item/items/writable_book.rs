use std::any::Any;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_util::Hand;

use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use verdantgolem_data::item::Item;
use verdantgolem_data::sound::{Sound, SoundCategory};
use verdantgolem_protocol::codec::var_int::VarInt;
use verdantgolem_protocol::java::client::play::COpenBook;

pub struct WritableBookItem;

impl ItemMetadata for WritableBookItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::WRITABLE_BOOK.id, Item::WRITTEN_BOOK.id])
    }
}

impl ItemBehaviour for WritableBookItem {
    fn normal_use(&self, stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        if stack.item.id == Item::WRITTEN_BOOK.id {
            player.try_send_client_packet(&COpenBook::new(VarInt(i32::from(hand == Hand::Left))));
            player.world().play_sound(
                Sound::ItemBookPageTurn,
                SoundCategory::Players,
                &player.position(),
            );
            return true;
        }
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
