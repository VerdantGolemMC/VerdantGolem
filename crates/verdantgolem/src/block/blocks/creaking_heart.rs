use std::sync::Arc;

use verdantgolem_data::block_properties::{
    Axis, BlockProperties, CreakingHeartLikeProperties, CreakingHeartState,
};
use verdantgolem_data::item::Item;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_data::sound::{Sound, SoundCategory};
use verdantgolem_data::{BlockId, BlockStateId};
use verdantgolem_macros::pumpkin_block;
use verdantgolem_world::world::BlockFlags;

use crate::block::entities::creaking_heart::CreakingHeartBlockEntity;
use crate::block::{BlockBehaviour, BrokenArgs, OnNeighborUpdateArgs, OnPlaceArgs, PlacedArgs};

#[pumpkin_block("minecraft:creaking_heart")]
pub struct CreakingHeartBlock;

impl CreakingHeartBlock {
    const fn is_pale_oak_log(id: BlockId) -> bool {
        matches!(id, BlockId::PALE_OAK_LOG | BlockId::STRIPPED_PALE_OAK_LOG)
    }

    fn check_active_logs(
        world: &dyn verdantgolem_world::world::BlockAccessor,
        pos: &verdantgolem_util::math::position::BlockPos,
        axis: Axis,
    ) -> bool {
        let (pos_a, pos_b) = match axis {
            Axis::X => (pos.west(), pos.east()),
            Axis::Y => (pos.down(), pos.up()),
            Axis::Z => (pos.north(), pos.south()),
        };

        let block_a = world.get_block_state_id(&pos_a).to_block_id();
        let block_b = world.get_block_state_id(&pos_b).to_block_id();

        Self::is_pale_oak_log(block_a) && Self::is_pale_oak_log(block_b)
    }
}

impl BlockBehaviour for CreakingHeartBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props =
            CreakingHeartLikeProperties::from_state_id(args.block.default_state.id, args.block);
        props.axis =
            match args.direction {
                verdantgolem_data::BlockDirection::North
                | verdantgolem_data::BlockDirection::South => Axis::Z,
                verdantgolem_data::BlockDirection::East
                | verdantgolem_data::BlockDirection::West => Axis::X,
                verdantgolem_data::BlockDirection::Up | verdantgolem_data::BlockDirection::Down => {
                    Axis::Y
                }
            };
        props.creaking_heart_state = CreakingHeartState::Uprooted;
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let entity = CreakingHeartBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));

            let state_id = args.world.get_block_state_id(args.position);
            let mut props = CreakingHeartLikeProperties::from_state_id(state_id, args.block);

            if Self::check_active_logs(args.world.as_ref(), args.position, props.axis) {
                props.creaking_heart_state = CreakingHeartState::Dormant;
                args.world.set_block_state(
                    args.position,
                    props.to_state_id(args.block),
                    BlockFlags::NOTIFY_ALL,
                );

                args.world.play_sound(
                    Sound::BlockCreakingHeartSpawn,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                );
            }
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        {
            let state_id = args.world.get_block_state_id(args.position);
            let mut props = CreakingHeartLikeProperties::from_state_id(state_id, args.block);

            let active_logs =
                Self::check_active_logs(args.world.as_ref(), args.position, props.axis);
            let new_state = if active_logs {
                if props.creaking_heart_state == CreakingHeartState::Uprooted {
                    args.world.play_sound(
                        Sound::BlockCreakingHeartSpawn,
                        SoundCategory::Blocks,
                        &args.position.to_f64(),
                    );
                }
                CreakingHeartState::Dormant
            } else {
                CreakingHeartState::Uprooted
            };

            if props.creaking_heart_state != new_state {
                props.creaking_heart_state = new_state;
                args.world.set_block_state(
                    args.position,
                    props.to_state_id(args.block),
                    BlockFlags::NOTIFY_ALL,
                );
            }
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        {
            args.world.play_sound(
                Sound::BlockCreakingHeartBreak,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );
            args.world
                .drop_stack(args.position, ItemStack::new(1, &Item::CREAKING_HEART));
        }
    }
}
