use crate::entity::player::Player;
use crate::item::ItemBehaviour;
use crate::item::ItemMetadata;
use crate::item::set_item_in_hand;
use std::any::Any;
use verdantgolem_data::data_component::DataComponent;
use verdantgolem_data::data_component_impl::DataComponentImpl;
use verdantgolem_data::data_component_impl::MapIdImpl;
use verdantgolem_data::item::Item;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_util::GameMode;
use verdantgolem_util::Hand;

pub struct MapItem;

impl ItemMetadata for MapItem {
    fn ids() -> Box<[u16]> {
        [Item::MAP.id].into()
    }
}

impl ItemBehaviour for MapItem {
    fn normal_use(&self, stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let Some(server) = player.world().server.upgrade() else {
            return false;
        };

        if stack.is_empty() || stack.item.id != Item::MAP.id {
            return false;
        }
        let inventory = player.inventory();
        let mut hand_stack = stack.clone();
        {
            let map_id = server.next_map_id();
            let _ = server.map_manager.create_map(
                map_id,
                player.world().dimension.clone(),
                player.position().x as i32,
                player.position().z as i32,
                0, // Default scale
            );

            let mut filled_map = ItemStack::new(1, &Item::FILLED_MAP);
            filled_map.patch.push((
                DataComponent::MapId,
                Some(MapIdImpl { id: map_id }.to_dyn()),
            ));

            let gamemode = player.gamemode.load();
            if hand_stack.item_count == 1 && gamemode != GameMode::Creative {
                set_item_in_hand(player, hand, filled_map);
            } else {
                hand_stack.decrement_unless_creative(gamemode, 1);
                set_item_in_hand(player, hand, hand_stack);
                let mut stack_to_give = filled_map;
                let was_added = inventory.insert_stack_anywhere(&mut stack_to_give);
                if !was_added && !stack_to_give.is_empty() {
                    player
                        .world()
                        .drop_stack(&player.position().to_block_pos(), stack_to_give);
                }
            }
        }
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
