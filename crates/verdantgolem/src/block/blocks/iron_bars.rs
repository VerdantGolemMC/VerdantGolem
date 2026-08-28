use crate::block::GetStateForNeighborUpdateArgs;
use crate::block::OnPlaceArgs;
use verdantgolem_data::BlockDirection;
use verdantgolem_data::BlockStateId;
use verdantgolem_data::HorizontalFacingExt;
use verdantgolem_data::block_properties::BlockProperties;
use verdantgolem_data::block_properties::HorizontalFacing;
use verdantgolem_data::tag::Taggable;
use verdantgolem_data::{Block, tag};
use verdantgolem_macros::pumpkin_block;
use verdantgolem_util::math::position::BlockPos;

type IronBarsProperties = verdantgolem_data::block_properties::OakFenceLikeProperties;

use crate::block::BlockBehaviour;
use crate::world::World;

#[pumpkin_block("minecraft:iron_bars")]
pub struct IronBarsBlock;

impl BlockBehaviour for IronBarsBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut bars_props = IronBarsProperties::default(args.block);
        bars_props.waterlogged = args.replacing.water_source();

        compute_bars_state(bars_props, args.world, args.block, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let bars_props = IronBarsProperties::from_state_id(args.state_id, args.block);
        compute_bars_state(bars_props, args.world, args.block, args.position)
    }
}

pub fn compute_bars_state(
    mut bars_props: IronBarsProperties,
    world: &World,
    block: &Block,
    block_pos: &BlockPos,
) -> BlockStateId {
    for direction in BlockDirection::horizontal() {
        let other_block_pos = block_pos.offset(direction.to_offset());
        let (other_block, other_block_state) = world.get_block_and_state(&other_block_pos);

        let connected = other_block == block
            || other_block_state.is_side_solid(direction.opposite().to_block_direction())
            || other_block.has_tag(&tag::Block::C_GLASS_PANES)
            || other_block.has_tag(&tag::Block::MINECRAFT_WALLS);

        match direction {
            HorizontalFacing::North => bars_props.north = connected,
            HorizontalFacing::South => bars_props.south = connected,
            HorizontalFacing::West => bars_props.west = connected,
            HorizontalFacing::East => bars_props.east = connected,
        }
    }

    bars_props.to_state_id(block)
}
