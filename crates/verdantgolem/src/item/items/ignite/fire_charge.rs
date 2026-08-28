use verdantgolem_data::BlockDirection;
use verdantgolem_data::item::Item;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_data::sound::Sound;
use verdantgolem_data::sound::SoundCategory;
use verdantgolem_data::{Block, BlockStateId};
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_util::math::vector3::Vector3;
use verdantgolem_world::world::BlockFlags;
use std::sync::Arc;

use crate::entity::player::Player;
use crate::item::items::ignite::ignition::Ignition;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use crate::world::World;

pub struct FireChargeItem;

impl ItemMetadata for FireChargeItem {
    fn ids() -> Box<[u16]> {
        [Item::FIRE_CHARGE.id].into()
    }
}

impl ItemBehaviour for FireChargeItem {
    fn use_on_block(
        &self,
        _item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        _server: &Server,
    ) {
        let world = player.world();
        Ignition::ignite_block(
            |world: Arc<World>, pos: BlockPos, new_state_id: BlockStateId| {
                world.set_block_state(&pos, new_state_id, BlockFlags::NOTIFY_ALL);

                world.play_block_sound(Sound::ItemFirechargeUse, SoundCategory::Blocks, pos);
            },
            &world,
            location,
            location.offset(face.to_offset()),
            block,
        );
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
