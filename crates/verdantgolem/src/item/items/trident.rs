use std::any::Any;
use std::sync::Arc;
use verdantgolem_util::Hand;

use crate::entity::player::Player;
use crate::entity::projectile::arrow::ArrowPickup;
use crate::entity::projectile::trident::TridentEntity;
use crate::entity::{Entity, EntityBase};
use crate::item::{ItemBehaviour, ItemMetadata, consume_item_in_hand};
use verdantgolem_data::entity::EntityType;
use verdantgolem_data::item::Item;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_data::sound::Sound;
use verdantgolem_util::GameMode;
use verdantgolem_util::math::vector3::Vector3;

pub struct TridentItem;

impl ItemMetadata for TridentItem {
    fn ids() -> Box<[u16]> {
        [Item::TRIDENT.id].into()
    }
}

impl ItemBehaviour for TridentItem {
    fn normal_use(&self, stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        player
            .living_entity
            .set_active_hand(hand, stack.clone(), 72000);
        true
    }

    fn on_stopped_using(&self, used_stack: &ItemStack, player: &Player, hand: Hand) {
        let stack = player.inventory().get_stack_in_hand(hand);
        if stack.is_empty() || !stack.are_items_and_components_equal(used_stack) {
            return;
        }
        let use_ticks = player
            .living_entity
            .item_use_time
            .load(std::sync::atomic::Ordering::Relaxed);
        let use_ticks = 72000 - use_ticks;

        if use_ticks < 10 {
            return;
        }

        let world = player.world();
        // Check Riptide level
        let mut riptide_level = 0u32;
        if let Some(enchantments) =
            stack.get_data_component::<verdantgolem_data::data_component_impl::EnchantmentsImpl>()
        {
            for (enchantment, level) in enchantments.enchantment.iter() {
                if **enchantment == verdantgolem_data::Enchantment::RIPTIDE {
                    riptide_level = *level as u32;
                }
            }
        }

        if riptide_level > 0 {
            let in_water = world.get_block_state(&player.position().to_block_pos()).id
                == verdantgolem_data::Block::WATER.default_state.id;
            if !in_water {
                player.living_entity.clear_active_hand();
                return;
            }

            let f = f64::from(riptide_level);
            let (yaw, pitch) = player.rotation();
            let f_yaw = f32::to_radians(yaw);
            let f_pitch = f32::to_radians(pitch);

            let vx = f64::from(-f32::sin(f_yaw) * f32::cos(f_pitch));
            let vy = f64::from(-f32::sin(f_pitch));
            let vz = f64::from(f32::cos(f_yaw) * f32::cos(f_pitch));

            let sq = (vx * vx + vy * vy + vz * vz).sqrt();
            if sq > 0.0 {
                let mult = (1.0 + f * 0.75) / sq;
                player.living_entity.entity.velocity.store(Vector3::new(
                    vx * mult,
                    vy * mult,
                    vz * mult,
                ));
            }

            player.damage_item_in_hand(hand, 1);
            player.living_entity.clear_active_hand();
            return;
        }

        // Damage and remove the releasing hand's trident, never a same-ID item
        // in the other hand. The projectile keeps the updated durability.
        player.damage_item_in_hand(hand, 1);
        let thrown_stack = player.inventory().get_stack_in_hand(hand);
        if thrown_stack.is_empty() {
            player.living_entity.clear_active_hand();
            return;
        }
        // Normal throw - spawn thrown trident
        let (yaw, pitch) = player.rotation();
        let entity = Entity::new(world.clone(), player.position(), &EntityType::TRIDENT);
        let trident = TridentEntity::new_shot(
            entity,
            player.get_entity(),
            thrown_stack.copy_with_count(1),
            if player.gamemode.load() == GameMode::Creative {
                ArrowPickup::CreativeOnly
            } else {
                ArrowPickup::Allowed
            },
        );
        trident.set_velocity_from_rotation(pitch, yaw, 0.0, 2.5, 1.0);
        world.spawn_entity(Arc::new(trident));

        world.play_sound(
            Sound::ItemTridentThrow,
            verdantgolem_data::sound::SoundCategory::Players,
            &player.position(),
        );

        consume_item_in_hand(player, hand, 1);

        player.living_entity.clear_active_hand();
    }

    fn can_mine(&self, player: &Player) -> bool {
        player.gamemode.load() != GameMode::Creative
    }

    fn get_use_duration(&self) -> i32 {
        72000
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
