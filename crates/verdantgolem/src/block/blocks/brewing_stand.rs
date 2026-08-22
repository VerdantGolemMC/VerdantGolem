use std::sync::Arc;
use std::sync::Mutex;

use crate::block::{GetComparatorOutputArgs, PlacedArgs};
use crate::block::{
    registry::BlockActionResult,
    {BlockBehaviour, NormalUseArgs},
};

use crate::block::entities::brewing_stand::BrewingStandBlockEntity;
use verdantgolem_data::translation;
use verdantgolem_inventory::player::player_inventory::PlayerInventory;
use verdantgolem_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use verdantgolem_macros::pumpkin_block;
use verdantgolem_util::text::TextComponent;
use verdantgolem_world::inventory::Inventory;

struct BrewingScreenFactory(
    Arc<dyn Inventory>,
    Arc<dyn crate::block::entities::PropertyDelegate>,
);

impl ScreenHandlerFactory for BrewingScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let inventory = self.0.clone();
        verdantgolem_inventory::brewing::create_brewing(
            sync_id,
            player_inventory,
            inventory,
            &self.1,
        )
        .map(|handler| Arc::new(Mutex::new(handler)) as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        verdantgolem_macros::translate_cross!(
            translation::java::CONTAINER_BREWING,
            translation::bedrock::CONTAINER_BREWING
        )
    }
}

#[pumpkin_block("minecraft:brewing_stand")]
pub struct BrewingStandBlock;

impl BlockBehaviour for BrewingStandBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(inventory) = block_entity.clone().get_inventory()
            && let Some(pd) = block_entity.clone().to_property_delegate()
        {
            args.player.increment_stat(
                verdantgolem_data::statistic::StatisticCategory::Custom,
                verdantgolem_data::statistic::CustomStatistic::InteractWithBrewingstand as i32,
                1,
            );
            args.player
                .open_handled_screen(&BrewingScreenFactory(inventory, pd), Some(*args.position));
        }

        BlockActionResult::Success
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let be = BrewingStandBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(be));
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(inventory) = block_entity.get_inventory()
        {
            let mut bottles = 0u8;
            // Bottle slots are 0, 1, 2 in brewing stands
            for slot in 0..3 {
                let stack = inventory.get_stack(slot);
                if !stack.is_empty() {
                    bottles += 1;
                }
            }
            Some(bottles)
        } else {
            None
        }
    }
}
