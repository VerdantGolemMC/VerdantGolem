use crate::block::{BlockBehaviour, OnPlaceArgs};
use verdantgolem_data::BlockDirection;
use verdantgolem_data::BlockStateId;
use verdantgolem_data::block_properties::Axis;
use verdantgolem_data::block_properties::BlockProperties;
use verdantgolem_macros::pumpkin_block;

#[pumpkin_block("minecraft:iron_chain")]
pub struct ChainBlock;

impl BlockBehaviour for ChainBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props =
            verdantgolem_data::block_properties::IronChainLikeProperties::default(args.block);
        props.r#waterlogged = args.replacing.water_source();
        props.r#axis = match args.direction {
            BlockDirection::East | BlockDirection::West => Axis::X,
            BlockDirection::Up | BlockDirection::Down => Axis::Y,
            BlockDirection::North | BlockDirection::South => Axis::Z,
        };

        props.to_state_id(args.block)
    }
}
