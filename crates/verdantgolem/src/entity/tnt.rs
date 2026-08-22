use super::{Entity, EntityBase, living::LivingEntity};
use crate::server::Server;
use core::f32;
use std::{
    f64::consts::TAU,
    sync::atomic::{
        AtomicU32,
        Ordering::{self, Relaxed},
    },
};
use verdantgolem_data::Block;
use verdantgolem_protocol::{codec::var_int::VarInt, java::client::play::Metadata};
use verdantgolem_util::math::vector3::Vector3;

pub struct TNTEntity {
    entity: Entity,
    power: f32,
    fuse: AtomicU32,
}

impl TNTEntity {
    /// Launch velocity for a freshly primed TNT, honoring the carpet rules
    /// `hardcodeTNTangle` and `tntPrimerMomentumRemoved`.
    #[must_use]
    pub fn primer_velocity() -> Vector3<f64> {
        let rules = crate::carpet::values();
        let angle = if rules.hardcode_tnt_angle >= 0.0 {
            rules.hardcode_tnt_angle
        } else if rules.tnt_primer_momentum_removed {
            return Vector3::new(0.0, 0.2, 0.0);
        } else {
            rand::random::<f64>() * TAU
        };
        Vector3::new(-angle.sin() * 0.02, 0.2, -angle.cos() * 0.02)
    }

    pub const fn new(entity: Entity, power: f32, fuse: u32) -> Self {
        Self {
            entity,
            power,
            fuse: AtomicU32::new(fuse),
        }
    }
}

impl EntityBase for TNTEntity {
    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = &self.entity;

        let mut velo = entity.velocity.load();
        velo.y -= self.get_gravity();

        entity.move_entity(caller, velo);
        entity.tick_block_collisions(caller);

        // Read back what actually happened instead of reusing the pre-move
        // value: `move_entity` clamps on collision, and an explosion may have
        // pushed us while we were moving above
        let velo = entity.velocity.load();
        if entity.on_ground.load(Ordering::Relaxed) {
            entity.velocity.store(velo.multiply(0.7, -0.5, 0.7));
        } else {
            entity.velocity.store(velo.multiply(0.98, 0.98, 0.98));
        }

        if entity.velocity_dirty.swap(false, Ordering::SeqCst) {
            entity.send_pos_rot();
            entity.send_velocity();
        }

        // FIX: Prevent fuse underflow (vanilla parity)
        let fuse = self.fuse.load(Relaxed);

        if fuse <= 1 {
            // TNT explodes now
            self.entity.remove();
            let world = self.entity.world.load_full();
            let pos = self.entity.pos.load();
            let power = {
                let rules = crate::carpet::values();
                if rules.tnt_random_range >= 0.0 {
                    rules.tnt_random_range as f32
                } else {
                    self.power
                }
            };
            if world.level_info.load().game_rules.tnt_explodes {
                world.explode(pos, power, crate::world::ExplosionInteraction::Tnt);
            }
        } else {
            // carpet rule mergeTNT: fold stationary primed TNT into one entity.
            if crate::carpet::values().merge_tnt
                && entity.on_ground.load(Ordering::Relaxed)
                && fuse.is_multiple_of(20)
            {
                let world = entity.world.load();
                let entities = world.entities.load();
                let bounding_box = entity.bounding_box.load();
                for other in entities.iter() {
                    if let Some(other_tnt) = other.clone().get_tnt_entity() {
                        let other_id = other_tnt.entity.entity_id;
                        if other_id != entity.entity_id
                            && !other_tnt.entity.removed.load(Ordering::Relaxed)
                            && other_tnt
                                .entity
                                .bounding_box
                                .load()
                                .intersects(&bounding_box)
                        {
                            let other_fuse = other_tnt.fuse.load(Relaxed);
                            if other_fuse < self.fuse.load(Relaxed) {
                                self.fuse.store(other_fuse, Relaxed);
                            }
                            other_tnt.entity.remove();
                        }
                    }
                }
            }

            // Safe decrement
            self.fuse.store(fuse - 1, Relaxed);
            entity.update_fluid_state(caller);
        }
    }

    fn init_data_tracker(&self) {
        let velocity = Self::primer_velocity();
        self.entity.set_velocity(velocity);

        self.entity.send_meta_data(
            &[
                Metadata::new(
                    verdantgolem_data::tracked_data::tnt::FUSE_ID,
                    VarInt(self.fuse.load(Relaxed) as i32),
                ),
                Metadata::new(
                    verdantgolem_data::tracked_data::tnt::BLOCK_STATE_ID,
                    VarInt(i32::from(Block::TNT.default_state.id.as_u16())),
                ),
            ],
            None,
        );
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn get_tnt_entity(self: Arc<Self>) -> Option<Arc<TNTEntity>> {
        Some(self)
    }

    fn get_gravity(&self) -> f64 {
        0.04
    }
    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
