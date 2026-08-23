use super::{Entity, EntityBase, living::LivingEntity};
use crate::server::Server;
use core::f32;
use std::{
    f64::consts::TAU,
    sync::{
        Arc,
        atomic::{
            AtomicU32,
            Ordering::{self, AcqRel, Acquire, Relaxed},
        },
    },
};
use verdantgolem_data::Block;
use verdantgolem_protocol::{codec::var_int::VarInt, java::client::play::Metadata};
use verdantgolem_util::math::vector3::Vector3;

pub struct TNTEntity {
    entity: Entity,
    power: f32,
    fuse: AtomicU32,
    merged_count: AtomicU32,
}

#[derive(Clone, Copy, Debug)]
struct MergeSnapshot {
    position: Vector3<f64>,
    velocity: Vector3<f64>,
    on_ground: bool,
    fuse: u32,
    power_bits: u32,
}

impl MergeSnapshot {
    fn is_stationary(self) -> bool {
        self.on_ground && self.velocity.x == 0.0 && self.velocity.y == 0.0 && self.velocity.z == 0.0
    }

    fn compatible_with(self, other: Self) -> bool {
        self.is_stationary()
            && other.is_stationary()
            && self.position == other.position
            && self.fuse == other.fuse
            && self.power_bits == other.power_bits
    }
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
            merged_count: AtomicU32::new(1),
        }
    }

    fn merge_snapshot(&self) -> MergeSnapshot {
        MergeSnapshot {
            position: self.entity.pos.load(),
            velocity: self.entity.velocity.load(),
            on_ground: self.entity.on_ground.load(Relaxed),
            fuse: self.fuse.load(Relaxed),
            power_bits: self.power.to_bits(),
        }
    }

    fn add_merged_count(&self, amount: u32) {
        let mut current = self.merged_count.load(Acquire);
        loop {
            let next = current.saturating_add(amount);
            match self
                .merged_count
                .compare_exchange_weak(current, next, AcqRel, Acquire)
            {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }

    fn try_merge_stationary_tnt(&self) {
        let own_snapshot = self.merge_snapshot();
        if !own_snapshot.is_stationary() || self.merged_count.load(Acquire) == 0 {
            return;
        }

        let self_id = self.entity.entity_id;
        let candidates = {
            let world = self.entity.world.load();
            let entities = world.entities.load();
            entities
                .iter()
                .filter_map(|candidate| candidate.clone().get_tnt_entity())
                .filter(|candidate| {
                    candidate.entity.entity_id != self_id
                        && !candidate.entity.removed.load(Relaxed)
                        && own_snapshot.compatible_with(candidate.merge_snapshot())
                        && candidate.merged_count.load(Acquire) > 0
                })
                .collect::<Vec<_>>()
        };

        // Only the lowest entity id in an equivalent merge group may absorb peers.
        // This prevents two concurrently ticking TNT entities from removing each other.
        if candidates
            .iter()
            .any(|candidate| candidate.entity.entity_id < self_id)
        {
            return;
        }

        for candidate in candidates {
            if self.entity.removed.load(Relaxed) {
                return;
            }
            let absorbed = candidate.merged_count.swap(0, AcqRel);
            if absorbed == 0 {
                continue;
            }
            self.add_merged_count(absorbed);
            candidate.entity.remove();
        }
    }
}

impl EntityBase for TNTEntity {
    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        let entity = &self.entity;

        let mut velocity = entity.velocity.load();
        velocity.y -= self.get_gravity();
        entity.move_entity(caller, velocity);
        entity.tick_block_collisions(caller);

        // Read back the post-collision velocity before applying vanilla drag.
        let velocity = entity.velocity.load();
        if entity.on_ground.load(Ordering::Relaxed) {
            entity.velocity.store(velocity.multiply(0.7, -0.5, 0.7));
        } else {
            entity.velocity.store(velocity.multiply(0.98, 0.98, 0.98));
        }

        if entity.velocity_dirty.swap(false, Ordering::SeqCst) {
            entity.send_pos_rot();
            entity.send_velocity();
        }

        // Carpet rule mergeTNT: only the lowest-id entity absorbs identical,
        // stationary peers, preserving the number of eventual explosions.
        if crate::carpet::values().merge_tnt
            && entity.on_ground.load(Ordering::Relaxed)
            && self.fuse.load(Relaxed).is_multiple_of(20)
        {
            self.try_merge_stationary_tnt();
            if entity.removed.load(Relaxed) || self.merged_count.load(Acquire) == 0 {
                return;
            }
        }

        // Prevent fuse underflow while retaining every merged explosion.
        let fuse = self.fuse.load(Relaxed);
        if fuse <= 1 {
            entity.remove();
            let explosion_count = self.merged_count.swap(0, AcqRel);
            if explosion_count == 0 {
                return;
            }
            let world = entity.world.load_full();
            if world.level_info.load().game_rules.tnt_explodes {
                let random_factor = {
                    let rules = crate::carpet::values();
                    (rules.tnt_random_range >= 0.0).then_some(rules.tnt_random_range as f32)
                };
                let position = entity.pos.load();
                for _ in 0..explosion_count {
                    world.explode_tnt_with_random_factor(position, self.power, random_factor);
                }
            }
        } else {
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

#[cfg(test)]
mod tests {
    use super::MergeSnapshot;
    use verdantgolem_util::math::vector3::Vector3;

    fn snapshot(position: Vector3<f64>, velocity: Vector3<f64>, fuse: u32) -> MergeSnapshot {
        MergeSnapshot {
            position,
            velocity,
            on_ground: true,
            fuse,
            power_bits: 4.0f32.to_bits(),
        }
    }

    #[test]
    fn merge_requires_identical_stationary_state() {
        let position = Vector3::new(1.0, 2.0, 3.0);
        let stationary = Vector3::new(0.0, 0.0, 0.0);
        let own = snapshot(position, stationary, 40);

        assert!(own.compatible_with(snapshot(position, stationary, 40)));
        assert!(!own.compatible_with(snapshot(Vector3::new(1.01, 2.0, 3.0), stationary, 40)));
        assert!(!own.compatible_with(snapshot(position, Vector3::new(0.01, 0.0, 0.0), 40)));
        assert!(!own.compatible_with(snapshot(position, stationary, 39)));

        let mut airborne = own;
        airborne.on_ground = false;
        assert!(!own.compatible_with(airborne));
    }
}
