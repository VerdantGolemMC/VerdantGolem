use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, NormalUseArgs, OnPlaceArgs};
use crate::entity::EntityBase;

use verdantgolem_data::block_properties::{BlockProperties, WallTorchLikeProperties};
use verdantgolem_data::translation;
use verdantgolem_data::{BlockStateId, FacingExt};
use verdantgolem_inventory::loom_screen_handler::LoomScreenHandler;
use verdantgolem_inventory::player::player_inventory::PlayerInventory;
use verdantgolem_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use verdantgolem_macros::pumpkin_block;
use verdantgolem_util::text::TextComponent;
use std::sync::Arc;
use std::sync::Mutex;

#[pumpkin_block("minecraft:loom")]
pub struct LoomBlock;

impl BlockBehaviour for LoomBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = WallTorchLikeProperties::default(args.block);
        if let Some(facing) = args
            .player
            .get_entity()
            .get_facing()
            .opposite()
            .to_horizontal_facing()
        {
            props.facing = facing;
        }
        props.to_state_id(args.block)
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        args.player.increment_stat(
            verdantgolem_data::statistic::StatisticCategory::Custom,
            verdantgolem_data::statistic::CustomStatistic::InteractWithLoom as i32,
            1,
        );
        args.player
            .open_handled_screen(&LoomScreenFactory, Some(*args.position));

        BlockActionResult::Success
    }
}

struct LoomScreenFactory;

impl ScreenHandlerFactory for LoomScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let handler: SharedScreenHandler = Arc::new(Mutex::new(LoomScreenHandler::new(
            sync_id,
            player_inventory,
        )));
        Some(handler)
    }

    fn get_display_name(&self) -> TextComponent {
        TextComponent::translate(translation::java::CONTAINER_LOOM, [])
    }
}
