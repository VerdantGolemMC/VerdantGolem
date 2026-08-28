use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, NormalUseArgs};

use verdantgolem_data::translation;
use verdantgolem_inventory::cartography_table_screen_handler::CartographyTableScreenHandler;
use verdantgolem_inventory::player::player_inventory::PlayerInventory;
use verdantgolem_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use verdantgolem_macros::pumpkin_block;
use verdantgolem_util::text::TextComponent;
use std::sync::Arc;
use std::sync::Mutex;

#[pumpkin_block("minecraft:cartography_table")]
pub struct CartographyTableBlock;

impl BlockBehaviour for CartographyTableBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        args.player.increment_stat(
            verdantgolem_data::statistic::StatisticCategory::Custom,
            verdantgolem_data::statistic::CustomStatistic::InteractWithCartographyTable as i32,
            1,
        );
        args.player
            .open_handled_screen(&CartographyTableScreenFactory, Some(*args.position));

        BlockActionResult::Success
    }
}

struct CartographyTableScreenFactory;

impl ScreenHandlerFactory for CartographyTableScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let handler: SharedScreenHandler = Arc::new(Mutex::new(
            CartographyTableScreenHandler::new(sync_id, player_inventory),
        ));
        Some(handler)
    }

    fn get_display_name(&self) -> TextComponent {
        TextComponent::translate(translation::java::CONTAINER_CARTOGRAPHY_TABLE, [])
    }
}
