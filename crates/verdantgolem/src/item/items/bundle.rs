use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata, set_item_in_hand};
use verdantgolem_data::data_component_impl::BundleContentsImpl;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_data::sound::Sound;
use verdantgolem_data::tag;
use verdantgolem_util::Hand;

pub struct BundleItem;

impl ItemMetadata for BundleItem {
    fn ids() -> Box<[u16]> {
        tag::Item::MINECRAFT_BUNDLES.1.into()
    }
}

impl ItemBehaviour for BundleItem {
    fn normal_use(&self, stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let mut bundle = stack.clone();
        let Some(contents) = bundle.get_data_component_mut::<BundleContentsImpl>() else {
            return false;
        };
        let Some(extracted_stack) = contents.try_extract() else {
            return false;
        };
        player.world().play_sound(
            Sound::ItemBundleRemoveOne,
            verdantgolem_data::sound::SoundCategory::Players,
            &player.position(),
        );
        set_item_in_hand(player, hand, bundle);
        player.drop_item(extracted_stack);
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
