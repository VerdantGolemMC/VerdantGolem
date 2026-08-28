use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use verdantgolem_data::tag;
use verdantgolem_util::GameMode;

pub struct SwordItem;

impl ItemMetadata for SwordItem {
    fn ids() -> Box<[u16]> {
        tag::Item::MINECRAFT_SWORDS.1.into()
    }
}

impl ItemBehaviour for SwordItem {
    fn can_mine(&self, player: &Player) -> bool {
        player.gamemode.load() != GameMode::Creative
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
