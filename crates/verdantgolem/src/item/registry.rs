use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::server::Server;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use verdantgolem_data::Block;
use verdantgolem_data::BlockDirection;
use verdantgolem_data::item::Item;
use verdantgolem_data::item_stack::ItemStack;
use verdantgolem_util::Hand;
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_util::math::vector3::Vector3;

use super::{ItemBehaviour, ItemMetadata};

#[derive(Default)]
pub struct ItemRegistry {
    items: FxHashMap<u16, Arc<dyn ItemBehaviour>>,
}

impl ItemRegistry {
    pub fn register<T: ItemBehaviour + ItemMetadata + 'static>(&mut self, item: T) {
        let val = Arc::new(item);
        self.items.reserve(T::ids().len());
        for i in T::ids() {
            self.items.insert(i, val.clone());
        }
    }

    /// Returns whether the item handled the interaction, rather than merely having a behaviour.
    pub fn on_use(&self, stack: &ItemStack, player: &Player, hand: Hand) -> bool {
        let item = stack.item;
        let cooldown = stack.get_use_cooldown();
        let cooldown_group = cooldown
            .and_then(|c| c.cooldown_group.clone())
            .unwrap_or_else(|| item.registry_key.to_string());

        self.dispatch_use(
            item.id,
            hand,
            player.is_on_cooldown(&cooldown_group),
            |behaviour, used_hand| behaviour.normal_use(stack, player, used_hand),
            || {
                if let Some(cooldown) = cooldown {
                    player.start_cooldown(cooldown_group, (cooldown.seconds * 20.0) as i32);
                }
            },
        )
    }

    pub fn on_stopped_using(&self, stack: &ItemStack, player: &Player, hand: Hand) {
        if let Some(behaviour) = self.get_pumpkin_item(stack.item.id) {
            behaviour.on_stopped_using(stack, player, hand);
        }
    }

    /// Returns the item's use duration in ticks, as defined by its registered behaviour.
    /// Returns `None` if the item has no registered behaviour or its duration is 0.
    #[must_use]
    pub fn get_use_duration(&self, item_id: u16) -> Option<i32> {
        self.get_pumpkin_item(item_id)
            .map(|b| b.get_use_duration())
            .filter(|&d| d > 0)
    }

    #[expect(clippy::too_many_arguments)]
    pub fn use_on_block(
        &self,
        stack: &mut ItemStack,
        player: &Player,
        hand: Hand,
        location: BlockPos,
        face: BlockDirection,
        cursor_pos: Vector3<f32>,
        block: &Block,
        server: &Server,
    ) -> bool {
        let cooldown = stack.get_use_cooldown().cloned();
        let cooldown_group = cooldown
            .as_ref()
            .and_then(|c| c.cooldown_group.clone())
            .unwrap_or_else(|| stack.item.registry_key.to_string());

        self.dispatch_use(
            stack.item.id,
            hand,
            player.is_on_cooldown(&cooldown_group),
            |behaviour, used_hand| {
                behaviour.use_on_block(
                    stack, player, used_hand, location, face, cursor_pos, block, server,
                )
            },
            || {
                if let Some(cooldown) = cooldown {
                    player.start_cooldown(cooldown_group, (cooldown.seconds * 20.0) as i32);
                }
            },
        )
    }

    pub fn use_on_entity(
        &self,
        stack: &mut ItemStack,
        player: &Player,
        hand: Hand,
        entity: Arc<dyn EntityBase>,
    ) -> bool {
        let cooldown = stack.get_use_cooldown().cloned();
        let cooldown_group = cooldown
            .as_ref()
            .and_then(|c| c.cooldown_group.clone())
            .unwrap_or_else(|| stack.item.registry_key.to_string());

        self.dispatch_use(
            stack.item.id,
            hand,
            player.is_on_cooldown(&cooldown_group),
            |behaviour, used_hand| behaviour.use_on_entity(stack, player, used_hand, entity),
            || {
                if let Some(cooldown) = cooldown {
                    player.start_cooldown(cooldown_group, (cooldown.seconds * 20.0) as i32);
                }
            },
        )
    }

    fn dispatch_use(
        &self,
        item_id: u16,
        hand: Hand,
        on_cooldown: bool,
        use_item: impl FnOnce(&dyn ItemBehaviour, Hand) -> bool,
        start_cooldown: impl FnOnce(),
    ) -> bool {
        if on_cooldown {
            return false;
        }

        let consumed = self
            .get_pumpkin_item(item_id)
            .is_some_and(|behaviour| use_item(behaviour.as_ref(), hand));
        if consumed {
            start_cooldown();
        }
        consumed
    }

    pub fn can_mine(&self, item: &Item, player: &Player) -> bool {
        let pumpkin_block = self.get_pumpkin_item(item.id);
        if let Some(pumpkin_block) = pumpkin_block {
            return pumpkin_block.can_mine(player);
        }
        true
    }

    #[must_use]
    pub fn get_pumpkin_item(&self, item: u16) -> Option<&Arc<dyn ItemBehaviour>> {
        self.items.get(&item)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    struct TestItem;

    impl ItemMetadata for TestItem {
        fn ids() -> Box<[u16]> {
            [Item::STICK.id].into()
        }
    }

    impl ItemBehaviour for TestItem {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    #[test]
    fn unregistered_items_pass_without_using_or_starting_cooldown() {
        let registry = ItemRegistry::default();
        assert!(!registry.dispatch_use(
            Item::STICK.id,
            Hand::Right,
            false,
            |_, _| panic!("unregistered item must not invoke a use callback"),
            || panic!("unregistered item must not start a cooldown"),
        ));
    }

    #[test]
    fn cooldown_prevents_use_callback_and_is_not_restarted() {
        let mut registry = ItemRegistry::default();
        registry.register(TestItem);
        assert!(!registry.dispatch_use(
            Item::STICK.id,
            Hand::Right,
            true,
            |_, _| panic!("an item on cooldown must not be used"),
            || panic!("an existing cooldown must not be restarted"),
        ));
    }

    #[test]
    fn passing_callback_does_not_start_cooldown() {
        let mut registry = ItemRegistry::default();
        registry.register(TestItem);
        let called = Cell::new(false);
        assert!(!registry.dispatch_use(
            Item::STICK.id,
            Hand::Right,
            false,
            |_, _| {
                called.set(true);
                false
            },
            || panic!("a passing callback must not start a cooldown"),
        ));
        assert!(called.get());
    }

    #[test]
    fn consumed_callback_starts_cooldown_after_use() {
        let mut registry = ItemRegistry::default();
        registry.register(TestItem);
        let used = Cell::new(false);
        let cooldown_started = Cell::new(false);
        assert!(registry.dispatch_use(
            Item::STICK.id,
            Hand::Right,
            false,
            |_, _| {
                assert!(!cooldown_started.get());
                used.set(true);
                true
            },
            || {
                assert!(used.get());
                cooldown_started.set(true);
            },
        ));
        assert!(cooldown_started.get());
    }
}
