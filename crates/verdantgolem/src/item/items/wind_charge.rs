use std::sync::Arc;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_util::Hand;

use crate::entity::player::Player;
use verdantgolem_data::entity::EntityType;
use verdantgolem_data::item::Item;
use verdantgolem_data::sound::Sound;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::projectile::ThrownItemEntity;
use crate::entity::projectile::wind_charge::{WIND_CHARGE_GRAVITY, WindChargeEntity};
use crate::item::{ItemBehaviour, ItemMetadata, consume_item_in_hand};

pub struct WindChargeItem;

impl ItemMetadata for WindChargeItem {
    fn ids() -> Box<[u16]> {
        [Item::WIND_CHARGE.id].into()
    }
}

const POWER: f32 = 1.5;

impl ItemBehaviour for WindChargeItem {
    fn normal_use(&self, _stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let world = player.world();
        let position = player.position();

        world.play_sound(
            Sound::EntityWindChargeThrow,
            verdantgolem_data::sound::SoundCategory::Neutral,
            &position,
        );

        let entity = Entity::new(world.clone(), position, &EntityType::WIND_CHARGE);

        let wind_charge = ThrownItemEntity::new(entity, player.get_entity(), WIND_CHARGE_GRAVITY);
        let (yaw, pitch) = player.rotation();

        wind_charge.set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);
        // TODO: player.incrementStat(Stats.USED)

        // TODO: Implement that the projectile will explode on impact
        world.spawn_entity(Arc::new(WindChargeEntity::new_normal(wind_charge)));
        consume_item_in_hand(player, hand, 1);
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
