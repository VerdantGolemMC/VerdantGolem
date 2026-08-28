use crate::block::{BlockBehaviour, OnPlaceArgs};
use verdantgolem_data::Block;
use verdantgolem_data::BlockStateId;
use verdantgolem_data::block_properties::BlockProperties;
use verdantgolem_data::block_properties::EndRodLikeProperties;
use verdantgolem_macros::pumpkin_block;

#[pumpkin_block("minecraft:end_rod")]
pub struct EndRodBlock;

impl BlockBehaviour for EndRodBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = EndRodLikeProperties::default(args.block);

        let blockstate = args
            .world
            .get_block_state_id(&args.position.offset(args.direction.to_offset()));

        if Block::from_state_id(blockstate).eq(args.block)
            && EndRodLikeProperties::from_state_id(blockstate, args.block).facing
                == args.direction.to_facing().opposite()
        {
            props.facing = args.direction.to_facing();
        } else {
            props.facing = args.direction.to_facing().opposite();
        }

        props.to_state_id(args.block)
    }
}
