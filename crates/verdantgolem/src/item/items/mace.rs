use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use verdantgolem_data::item::Item;
use verdantgolem_util::GameMode;

pub struct MaceItem;

impl ItemMetadata for MaceItem {
    fn ids() -> Box<[u16]> {
        [Item::MACE.id].into()
    }
}
impl ItemBehaviour for MaceItem {
    fn can_mine(&self, player: &Player) -> bool {
        player.gamemode.load() != GameMode::Creative
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
