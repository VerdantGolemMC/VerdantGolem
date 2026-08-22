use crate::block::blocks::falling::FallingBlock;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, GetStateForNeighborUpdateArgs, NormalUseArgs, OnPlaceArgs, OnScheduledTickArgs,
    PlacedArgs,
};

use std::sync::Arc;
use std::sync::Mutex;
use verdantgolem_data::BlockStateId;
use verdantgolem_data::block_properties::{BlockProperties, WallTorchLikeProperties};
use verdantgolem_data::translation;
use verdantgolem_inventory::anvil::AnvilScreenHandler;
use verdantgolem_inventory::player::player_inventory::PlayerInventory;
use verdantgolem_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use verdantgolem_macros::pumpkin_block_from_tag;
use verdantgolem_util::text::TextComponent;
use verdantgolem_world::inventory::SimpleInventory;

#[pumpkin_block_from_tag("minecraft:anvil")]
pub struct AnvilBlock;

impl BlockBehaviour for AnvilBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        args.player.increment_stat(
            verdantgolem_data::statistic::StatisticCategory::Custom,
            verdantgolem_data::statistic::CustomStatistic::InteractWithAnvil as i32,
            1,
        );
        args.player
            .open_handled_screen(&AnvilScreenFactory, Some(*args.position));

        BlockActionResult::Success
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        FallingBlock::placed(&FallingBlock, args);
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let dir = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .rotate_clockwise();

        let mut props = WallTorchLikeProperties::default(args.block);

        props.facing = dir;
        props.to_state_id(args.block)
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        FallingBlock::on_scheduled_tick(&FallingBlock, args);
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        FallingBlock::get_state_for_neighbor_update(&FallingBlock, args)
    }
}

struct AnvilScreenFactory;

impl ScreenHandlerFactory for AnvilScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let inventory = Arc::new(SimpleInventory::new(3));
        let handler = AnvilScreenHandler::new(sync_id, player_inventory, inventory);
        let concrete_arc = Arc::new(Mutex::new(handler));

        Some(concrete_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        verdantgolem_macros::translate_cross!(
            translation::java::CONTAINER_REPAIR,
            translation::bedrock::CONTAINER_REPAIR
        )
    }
}
