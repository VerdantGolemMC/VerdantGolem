use std::any::Any;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_util::Hand;

use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use verdantgolem_data::item::Item;

pub struct CarrotOnAStickItem;
pub struct WarpedFungusOnAStickItem;

impl ItemMetadata for CarrotOnAStickItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::CARROT_ON_A_STICK.id])
    }
}

impl ItemBehaviour for CarrotOnAStickItem {
    fn normal_use(&self, _stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let vehicle_opt = player
            .get_entity()
            .vehicle
            .try_lock()
            .ok()
            .and_then(|guard| guard.clone());
        if let Some(vehicle) = vehicle_opt
            && let Some(steerable) = vehicle.get_item_steerable()
            && steerable.boost()
        {
            player.damage_item_in_hand(hand, 7);
            return true;
        }
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ItemMetadata for WarpedFungusOnAStickItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::WARPED_FUNGUS_ON_A_STICK.id])
    }
}

impl ItemBehaviour for WarpedFungusOnAStickItem {
    fn normal_use(&self, _stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let vehicle_opt = player
            .get_entity()
            .vehicle
            .try_lock()
            .ok()
            .and_then(|guard| guard.clone());
        if let Some(vehicle) = vehicle_opt
            && let Some(steerable) = vehicle.get_item_steerable()
            && steerable.boost()
        {
            player.damage_item_in_hand(hand, 7);
            return true;
        }
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
