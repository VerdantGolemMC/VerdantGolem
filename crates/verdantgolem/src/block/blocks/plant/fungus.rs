use crate::block::{BlockBehaviour, BlockMetadata, CanPlaceAtArgs};
use crate::block::{GetStateForNeighborUpdateArgs, blocks::plant::PlantBlockBase};
use verdantgolem_data::BlockStateId;
use verdantgolem_data::tag::Taggable;
use verdantgolem_data::{Block, BlockId, tag};
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_world::world::BlockAccessor;
pub struct FungusBlock;

impl BlockMetadata for FungusBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::CRIMSON_FUNGUS, BlockId::WARPED_FUNGUS].into()
    }
}

impl BlockBehaviour for FungusBlock {
    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        <Self as PlantBlockBase>::get_state_for_neighbor_update(
            self,
            args.world,
            args.position,
            args.state_id,
        )
    }
}
impl PlantBlockBase for FungusBlock {
    fn can_plant_on_top(
        &self,
        block_accessor: &dyn verdantgolem_world::world::BlockAccessor,
        pos: &verdantgolem_util::math::position::BlockPos,
    ) -> bool {
        let block = block_accessor.get_block(pos);

        if block == &Block::WARPED_FUNGUS {
            return block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_WARPED_FUNGUS);
        }
        block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CRIMSON_FUNGUS)
    }
    fn can_place_at(&self, block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        <Self as PlantBlockBase>::can_plant_on_top(self, block_accessor, &block_pos.down())
    }
}
