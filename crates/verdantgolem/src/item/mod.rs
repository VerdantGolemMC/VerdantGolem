pub mod items;
pub mod potion;
pub mod registry;

use std::any::Any;
use std::sync::Arc;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::server::Server;
use verdantgolem_data::Block;
use verdantgolem_data::BlockDirection;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_inventory::player::player_inventory::PlayerInventory;
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_util::math::vector3::Vector3;
use verdantgolem_util::{GameMode, Hand};

pub trait ItemMetadata {
    fn ids() -> Box<[u16]>;
}

pub trait ItemBehaviour: Send + Sync {
    /// Returns whether this interaction was handled and should stop use fallback.
    /// A cancelled interaction is handled too; unsupported uses return `false`.
    fn normal_use(&self, _stack: &ItemStack, _player: &Player, _hand: Hand) -> bool {
        false
    }

    /// Returns `true` when the block interaction was performed or explicitly cancelled.
    #[expect(clippy::too_many_arguments)]
    fn use_on_block(
        &self,
        _item: &mut ItemStack,
        _player: &Player,
        _hand: Hand,
        _location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        _block: &Block,
        _server: &Server,
    ) -> bool {
        false
    }

    /// Returns `true` when the entity interaction was performed or explicitly cancelled.
    fn use_on_entity(
        &self,
        _item: &mut ItemStack,
        _player: &Player,
        _hand: Hand,
        _entity: Arc<dyn EntityBase>,
    ) -> bool {
        false
    }

    fn on_stopped_using(&self, _stack: &ItemStack, _player: &Player, _hand: Hand) {}

    /// Returns the maximum number of ticks this item can be used for.
    /// Return 0 if the item does not have a behaviour-driven use duration.
    fn get_use_duration(&self) -> i32 {
        0
    }

    fn can_mine(&self, _player: &Player) -> bool {
        true
    }

    fn get_start_and_end_pos(&self, player: &Player) -> (Vector3<f64>, Vector3<f64>) {
        let start_pos = player.eye_position();
        let (yaw, pitch) = player.rotation();
        let (yaw_rad, pitch_rad) = (f64::from(yaw.to_radians()), f64::from(pitch.to_radians()));
        let block_interaction_range = 4.5; // This is not the same as the block_interaction_range in the
        // player entity.
        let direction = Vector3::new(
            -yaw_rad.sin() * pitch_rad.cos() * block_interaction_range,
            -pitch_rad.sin() * block_interaction_range,
            pitch_rad.cos() * yaw_rad.cos() * block_interaction_range,
        );

        let end_pos = start_pos.add(&direction);
        (start_pos, end_pos)
    }

    fn as_any(&self) -> &dyn Any;
}

/// Inventory mutations from normal-use callbacks must use the requested hand,
/// even when both hands contain the same item or the main hand is unrelated.
pub(crate) fn set_item_in_hand(player: &Player, hand: Hand, stack: ItemStack) {
    player.inventory().set_stack_in_hand(hand, stack.clone());
    player.sync_hand_slot(hand_slot_index(player.inventory(), hand), stack);
}

pub(crate) fn consume_item_in_hand(player: &Player, hand: Hand, amount: u8) {
    let stack = decrement_inventory_hand(player.inventory(), hand, player.gamemode.load(), amount);
    player.sync_hand_slot(hand_slot_index(player.inventory(), hand), stack);
}

fn hand_slot_index(inventory: &PlayerInventory, hand: Hand) -> usize {
    match hand {
        Hand::Right => inventory.get_selected_slot() as usize,
        Hand::Left => PlayerInventory::OFF_HAND_SLOT,
    }
}

fn decrement_inventory_hand(
    inventory: &PlayerInventory,
    hand: Hand,
    gamemode: GameMode,
    amount: u8,
) -> ItemStack {
    let mut stack = inventory.get_stack_in_hand(hand);
    stack.decrement_unless_creative(gamemode, amount);
    inventory.set_stack_in_hand(hand, stack.clone());
    stack
}

#[cfg(test)]
mod hand_tests {
    use super::*;
    use std::{collections::HashMap, sync::Mutex, sync::atomic::Ordering};
    use verdantgolem_data::{data_component_impl::EquipmentSlot, item::Item};
    use verdantgolem_inventory::entity_equipment::EntityEquipment;

    fn inventory() -> PlayerInventory {
        PlayerInventory::new(
            Arc::new(Mutex::new(EntityEquipment::new())),
            Arc::new(HashMap::from([(
                PlayerInventory::OFF_HAND_SLOT,
                EquipmentSlot::OFF_HAND,
            )])),
        )
    }

    #[test]
    fn offhand_experience_bottle_consumption_preserves_unrelated_mainhand() {
        let inventory = inventory();
        inventory.selected_slot.store(4, Ordering::Relaxed);
        let main = ItemStack::new(12, &Item::STICK);
        inventory.set_stack_in_hand(Hand::Right, main.clone());
        inventory.set_stack_in_hand(Hand::Left, ItemStack::new(2, &Item::EXPERIENCE_BOTTLE));

        let remaining = decrement_inventory_hand(&inventory, Hand::Left, GameMode::Survival, 1);
        assert_eq!(
            hand_slot_index(&inventory, Hand::Left),
            PlayerInventory::OFF_HAND_SLOT
        );
        assert_eq!(remaining.item.id, Item::EXPERIENCE_BOTTLE.id);
        assert_eq!(remaining.item_count, 1);
        assert!(inventory.held_item().are_equal(&main));

        decrement_inventory_hand(&inventory, Hand::Left, GameMode::Survival, 1);
        assert!(inventory.off_hand_item().is_empty());
        assert!(inventory.held_item().are_equal(&main));
    }

    #[test]
    fn equal_items_in_both_hands_do_not_redirect_consumption() {
        let inventory = inventory();
        inventory.set_stack_in_hand(Hand::Right, ItemStack::new(6, &Item::SNOWBALL));
        inventory.set_stack_in_hand(Hand::Left, ItemStack::new(3, &Item::SNOWBALL));
        decrement_inventory_hand(&inventory, Hand::Left, GameMode::Survival, 1);
        assert_eq!(inventory.held_item().item_count, 6);
        assert_eq!(inventory.off_hand_item().item_count, 2);
        decrement_inventory_hand(&inventory, Hand::Right, GameMode::Survival, 1);
        assert_eq!(inventory.held_item().item_count, 5);
        assert_eq!(inventory.off_hand_item().item_count, 2);
    }

    #[test]
    fn creative_use_keeps_the_requested_hand_count() {
        let inventory = inventory();
        let bottle = ItemStack::new(2, &Item::EXPERIENCE_BOTTLE);
        inventory.set_stack_in_hand(Hand::Left, bottle.clone());
        decrement_inventory_hand(&inventory, Hand::Left, GameMode::Creative, 1);
        assert!(inventory.off_hand_item().are_equal(&bottle));
        assert!(inventory.held_item().is_empty());
    }
}
