use std::sync::Arc;
use std::sync::Mutex;

use crate::block::blocks::redstone::block_receives_redstone_power;
use crate::block::{GetComparatorOutputArgs, OnNeighborUpdateArgs, OnPlaceArgs, PlacedArgs};
use crate::block::{
    registry::BlockActionResult,
    {BlockBehaviour, NormalUseArgs},
};
use crate::world::World;

use crate::block::entities::hopper::HopperBlockEntity;
use verdantgolem_data::BlockStateId;
use verdantgolem_data::block_properties::{BlockProperties, FacingHopper};
use verdantgolem_data::{Block, BlockDirection, translation};
use verdantgolem_inventory::generic_container_screen_handler::create_hopper;
use verdantgolem_inventory::player::player_inventory::PlayerInventory;
use verdantgolem_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use verdantgolem_macros::pumpkin_block;
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_util::text::TextComponent;
use verdantgolem_world::inventory::Inventory;
use verdantgolem_world::world::BlockFlags;

struct HopperBlockScreenFactory(Arc<dyn Inventory>);

impl ScreenHandlerFactory for HopperBlockScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let concrete_handler = create_hopper(sync_id, player_inventory, self.0.clone());
        let concrete_arc = Arc::new(Mutex::new(concrete_handler));

        Some(concrete_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        verdantgolem_macros::translate_cross!(
            translation::java::CONTAINER_HOPPER,
            translation::bedrock::CONTAINER_HOPPER
        )
    }
}

#[pumpkin_block("minecraft:hopper")]
pub struct HopperBlock;

type HopperLikeProperties = verdantgolem_data::block_properties::HopperLikeProperties;

impl BlockBehaviour for HopperBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(inventory) = block_entity.get_inventory()
        {
            args.player.increment_stat(
                verdantgolem_data::statistic::StatisticCategory::Custom,
                verdantgolem_data::statistic::CustomStatistic::InspectHopper as i32,
                1,
            );
            args.player
                .open_handled_screen(&HopperBlockScreenFactory(inventory), Some(*args.position));
        }

        BlockActionResult::Success
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = HopperLikeProperties::default(args.block);
        props.facing = match args.direction {
            BlockDirection::North => FacingHopper::North,
            BlockDirection::East => FacingHopper::East,
            BlockDirection::South => FacingHopper::South,
            BlockDirection::West => FacingHopper::West,
            BlockDirection::Up | BlockDirection::Down => FacingHopper::Down,
        };
        props.enabled = true;
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let props = HopperLikeProperties::from_state_id(args.state_id, args.block);
        let hopper_block_entity = HopperBlockEntity::new(*args.position, props.facing);
        args.world.add_block_entity(Arc::new(hopper_block_entity));
        if Block::from_state_id(args.old_state_id) != Block::from_state_id(args.state_id) {
            check_powered_state(args.world, args.position, args.state_id, args.block);
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        check_powered_state(
            args.world,
            args.position,
            args.world.get_block_state_id(args.position),
            args.block,
        );
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(inventory) = block_entity.get_inventory()
        {
            Some(crate::block::calculate_comparator_output(
                inventory.as_ref(),
            ))
        } else {
            None
        }
    }
}

fn check_powered_state(world: &Arc<World>, pos: &BlockPos, state_id: BlockStateId, block: &Block) {
    let signal = !block_receives_redstone_power(world, pos);
    let mut state = HopperLikeProperties::from_state_id(state_id, block);
    if signal != state.enabled {
        state.enabled = signal;
        world.set_block_state(pos, state.to_state_id(block), BlockFlags::NOTIFY_LISTENERS);
    }
}
