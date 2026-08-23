use std::sync::Arc;
use verdantgolem_data::item::Item;

use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, ExplodeArgs, OnNeighborUpdateArgs, PlacedArgs, UseWithItemArgs,
};
use crate::entity::Entity;
use crate::entity::tnt::TNTEntity;
use crate::world::World;
use rand::RngExt;
use verdantgolem_data::BlockStateId;
use verdantgolem_data::entity::EntityType;
use verdantgolem_data::sound::SoundCategory;
use verdantgolem_macros::pumpkin_block;
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_world::world::BlockFlags;

use super::redstone::block_receives_redstone_power;

#[pumpkin_block("minecraft:tnt")]
pub struct TNTBlock;

impl TNTBlock {
    pub fn prime(world: &Arc<World>, location: &BlockPos) {
        let mut event = crate::plugin::api::events::block::tnt_prime::TNTPrimeEvent::new(
            *location,
            "REDSTONE".to_string(),
        );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return;
        }

        let entity = Entity::new(world.clone(), location.to_f64(), &EntityType::TNT);
        let mut prime_event =
            crate::plugin::api::events::entity::explosion_prime::ExplosionPrimeEvent::new(
                entity.entity_id,
                DEFAULT_POWER,
                false,
            );
        if let Some(server) = world.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut prime_event);
        }
        if prime_event.cancelled {
            return;
        }

        let pos = entity.pos.load();
        let tnt = Arc::new(TNTEntity::new(entity, DEFAULT_POWER, DEFAULT_FUSE));
        world.spawn_entity(tnt);
        world.play_sound(
            verdantgolem_data::sound::Sound::EntityTntPrimed,
            SoundCategory::Blocks,
            &pos,
        );
        world.set_block_state(location, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
    }
}

const DEFAULT_FUSE: u32 = 80;
const DEFAULT_POWER: f32 = 4.0;

impl BlockBehaviour for TNTBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        {
            let item = args.item_stack.item;
            if item != &Item::FLINT_AND_STEEL || item == &Item::FIRE_CHARGE {
                return BlockActionResult::Pass;
            }
            let world = args.player.world();
            Self::prime(&world, args.position);

            BlockActionResult::Consume
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        // Carpet's tntDoNotUpdate only suppresses the placement-time power
        // check. Later neighbour changes must still be able to prime the TNT.
        if !crate::carpet::values().tnt_do_not_update
            && block_receives_redstone_power(args.world, args.position)
        {
            Self::prime(args.world, args.position);
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if block_receives_redstone_power(args.world, args.position) {
            Self::prime(args.world, args.position);
        }
    }

    fn explode(&self, args: ExplodeArgs<'_>) {
        {
            let entity = Entity::new(args.world.clone(), args.position.to_f64(), &EntityType::TNT);
            entity.set_velocity(TNTEntity::primer_velocity());
            let fuse = rand::rng().random_range(0..DEFAULT_FUSE / 4) + DEFAULT_FUSE / 8;
            let tnt = Arc::new(TNTEntity::new(entity, DEFAULT_POWER, fuse));
            args.world.spawn_entity(tnt);
        }
    }

    fn should_drop_items_on_explosion(&self) -> bool {
        false
    }
}
