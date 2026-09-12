//! Shared, protocol-independent player interactions. Java packets validate their
//! wire fields before entering these methods; local players supply raycast hits.

use std::sync::{Arc, atomic::Ordering};

use verdantgolem_data::{
    Block, BlockDirection,
    attributes::Attributes,
    data_component_impl::{
        BlocksAttacksImpl, ConsumableImpl, EquipmentSlot, EquippableImpl, FoodImpl,
    },
    item::Item,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
};
use verdantgolem_inventory::{
    player::player_inventory::PlayerInventory, screen_handler::InventoryPlayer,
};
use verdantgolem_protocol::{
    codec::var_int::VarInt,
    java::{client::play::CBlockUpdate, server::play::ActionType},
};
use verdantgolem_util::{
    GameMode, Hand,
    math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3},
};

use crate::{
    block::{BlockHitResult, BlockUseContext, registry::BlockActionResult},
    entity::{EntityBase, equipment_break_status},
    net::java::play::BlockPlacingError,
    plugin::{
        api::events::player::{
            player_interact_at_entity::PlayerInteractAtEntityEvent,
            player_item_consume::PlayerItemConsumeEvent,
        },
        player::{
            fish::{PlayerFishEvent, PlayerFishState},
            player_interact_entity_event::PlayerInteractEntityEvent,
            player_interact_event::{InteractAction, PlayerInteractEvent},
        },
    },
    server::Server,
    world::World,
};

use super::{Player, statistics::StatisticCategory};

/// A cancellation is handled, not a pass: callers must not fall through to a
/// second hand, item use, or an attack after a plugin veto.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionResult {
    Pass,
    Consumed,
    Blocked,
}

impl InteractionResult {
    pub(crate) const fn consumes_action(self) -> bool {
        !matches!(self, Self::Pass)
    }

    const fn from_used(used: bool) -> Self {
        if used { Self::Consumed } else { Self::Pass }
    }

    const fn from_block_result(result: &BlockActionResult) -> Self {
        match result {
            BlockActionResult::Success
            | BlockActionResult::SuccessServer
            | BlockActionResult::Consume => Self::Consumed,
            BlockActionResult::Fail => Self::Blocked,
            BlockActionResult::Pass | BlockActionResult::PassToDefaultBlockAction => Self::Pass,
        }
    }
}

pub enum LookTarget {
    Entity(Arc<dyn EntityBase>, Vector3<f64>),
    Block(BlockUseContext),
    Miss,
}

/// Parameter on a finite segment at which it enters the box, with the actual
/// outline face (not simply the face of the containing voxel).
fn ray_box_hit(
    start: Vector3<f64>,
    delta: Vector3<f64>,
    bounding_box: &BoundingBox,
) -> Option<(f64, BlockDirection)> {
    let origin = [start.x, start.y, start.z];
    let direction = [delta.x, delta.y, delta.z];
    let minimum = [bounding_box.min.x, bounding_box.min.y, bounding_box.min.z];
    let maximum = [bounding_box.max.x, bounding_box.max.y, bounding_box.max.z];
    let negative_faces = [
        BlockDirection::West,
        BlockDirection::Down,
        BlockDirection::North,
    ];
    let positive_faces = [
        BlockDirection::East,
        BlockDirection::Up,
        BlockDirection::South,
    ];
    let mut enter = 0.0f64;
    let mut exit = 1.0f64;
    let mut face = BlockDirection::Up;
    for axis in 0..3 {
        if !origin[axis].is_finite() || !direction[axis].is_finite() {
            return None;
        }
        if direction[axis].abs() < 1.0e-9 {
            if origin[axis] < minimum[axis] || origin[axis] > maximum[axis] {
                return None;
            }
            continue;
        }
        let first = (minimum[axis] - origin[axis]) / direction[axis];
        let last = (maximum[axis] - origin[axis]) / direction[axis];
        let near = first.min(last);
        if near >= enter {
            enter = near;
            face = if direction[axis] > 0.0 {
                negative_faces[axis]
            } else {
                positive_faces[axis]
            };
        }
        exit = exit.min(first.max(last));
        if exit < enter {
            return None;
        }
    }
    (0.0..=1.0).contains(&enter).then_some((enter, face))
}

/// Outline-aware DDA, shared with deterministic tests. `World::raycast` also
/// supports fluid/block fallback boxes; interaction selection must not turn an
/// empty AIR outline into an occluder.
fn trace_block_outlines(
    start: Vector3<f64>,
    end: Vector3<f64>,
    mut shapes_at: impl FnMut(BlockPos) -> Vec<BoundingBox>,
) -> Option<(f64, BlockUseContext)> {
    let origin = [start.x, start.y, start.z];
    let delta = end - start;
    let direction = [delta.x, delta.y, delta.z];
    if origin
        .iter()
        .chain(direction.iter())
        .any(|v| !v.is_finite())
        || delta.length_squared() > 128.0 * 128.0
        || delta.length_squared() < 1.0e-18
        || origin.iter().any(|v| v.abs() > 30_000_001.0)
    {
        return None;
    }
    let mut block = BlockPos::floored_v(start);
    let mut next = [f64::INFINITY; 3];
    let mut stride = [f64::INFINITY; 3];
    let mut step = [0; 3];
    for axis in 0..3 {
        if direction[axis].abs() >= 1.0e-9 {
            step[axis] = if direction[axis] > 0.0 { 1 } else { -1 };
            let boundary = origin[axis].floor() + if step[axis] > 0 { 1.0 } else { 0.0 };
            next[axis] = (boundary - origin[axis]) / direction[axis];
            stride[axis] = direction[axis].abs().recip();
        }
    }
    // A 128-block segment crosses at most ceil(128 * sqrt(3)) + 3 cells.
    // The explicit bound also protects against degenerate floating-point rays.
    for _ in 0..256 {
        if let Some((t, face)) = shapes_at(block)
            .iter()
            .filter_map(|shape| ray_box_hit(start, delta, shape))
            .min_by(|a, b| a.0.total_cmp(&b.0))
        {
            let relative = start + delta * t - block.0.to_f64();
            return Some((
                t,
                BlockUseContext {
                    position: block,
                    face,
                    cursor_pos: Vector3::new(
                        relative.x as f32,
                        relative.y as f32,
                        relative.z as f32,
                    ),
                },
            ));
        }
        let axis = if next[0] <= next[1] && next[0] <= next[2] {
            0
        } else if next[1] <= next[2] {
            1
        } else {
            2
        };
        if next[axis] > 1.0 {
            return None;
        }
        match axis {
            0 => block.0.x += step[0],
            1 => block.0.y += step[1],
            _ => block.0.z += step[2],
        }
        next[axis] += stride[axis];
    }
    None
}

fn block_hit_between(
    world: &Arc<World>,
    start: Vector3<f64>,
    end: Vector3<f64>,
) -> Option<(f64, BlockUseContext)> {
    trace_block_outlines(start, end, |position| {
        world
            .get_block_state(&position)
            .get_block_outline_shapes_at(&position)
            .map(|shape| shape.at_pos(position))
            .collect()
    })
}

/// `removed` is set before asynchronous player teardown, whereas the entity's
/// separate removal reason can still be unset while an old tick snapshot lives.
const fn entity_lifecycle_allows_actions(removal_requested: bool, entity_alive: bool) -> bool {
    !removal_requested && entity_alive
}

fn entity_within_reach(eye: Vector3<f64>, bounding_box: &BoundingBox, reach: f64) -> bool {
    bounding_box.squared_magnitude(eye) <= reach * reach
}

fn selectable_entity(target: &dyn EntityBase) -> bool {
    let entity = target.get_entity();
    if !entity_lifecycle_allows_actions(entity.removed.load(Ordering::Relaxed), entity.is_alive())
        || target.get_player().is_some_and(Player::is_spectator)
    {
        return false;
    }
    if let Some(living) = target.get_living_entity() {
        return living.health.load() > 0.0 && !living.dead.load(Ordering::Relaxed);
    }
    // The generic trait defaults to non-targetable. Hanging entities/crystals
    // are still interactive; drops, XP orbs, markers and arrows are not.
    target.can_hit()
        || matches!(
            entity.entity_type.resource_name,
            "item_frame" | "glow_item_frame" | "painting" | "end_crystal" | "leash_knot"
        )
}

fn hit_is_before_block(entity_t: f64, block_t: Option<f64>) -> bool {
    block_t.is_none_or(|block_t| entity_t < block_t)
}

fn use_in_hand_order(
    mut target_use: impl FnMut(Hand) -> InteractionResult,
    mut air_use: impl FnMut(Hand) -> InteractionResult,
) -> InteractionResult {
    for hand in [Hand::Right, Hand::Left] {
        let targeted = target_use(hand);
        if targeted.consumes_action() {
            return targeted;
        }
        let air = air_use(hand);
        if air.consumes_action() {
            return air;
        }
    }
    InteractionResult::Pass
}

impl Player {
    pub(crate) fn is_alive_for_actions(&self) -> bool {
        let entity = self.get_entity();
        entity_lifecycle_allows_actions(entity.removed.load(Ordering::Relaxed), entity.is_alive())
            && self.living_entity.health.load() > 0.0
            && !self.living_entity.dead.load(Ordering::Relaxed)
    }

    pub(crate) fn can_perform_actions(&self) -> bool {
        self.has_client_loaded()
            && self.is_alive_for_actions()
            && !self.is_spectator()
            && self.sleeping_since.load().is_none()
    }

    pub(crate) fn entity_interaction_range(&self) -> f64 {
        let range = self
            .living_entity
            .get_attribute_value(&Attributes::ENTITY_INTERACTION_RANGE);
        if self.gamemode.load() == GameMode::Creative {
            range + 2.0
        } else {
            range
        }
    }

    /// Raycast once so an entity masks the block behind it. Distinct entity and
    /// block ranges, partial block outlines, and block ties are all respected.
    pub(crate) fn look_target(&self) -> LookTarget {
        let entity = &self.living_entity.entity;
        let start = entity.get_eye_pos();
        let reach = self
            .block_interaction_range()
            .max(self.entity_interaction_range());
        let delta =
            Vector3::rotation_vector(f64::from(entity.pitch.load()), f64::from(entity.yaw.load()))
                * reach;
        let end = start + delta;
        let world = self.world();
        let block_hit = block_hit_between(&world, start, end);
        let block_t = block_hit.as_ref().map(|(t, _)| *t);
        let search = BoundingBox::new(
            Vector3::new(start.x.min(end.x), start.y.min(end.y), start.z.min(end.z)),
            Vector3::new(start.x.max(end.x), start.y.max(end.y), start.z.max(end.z)),
        )
        .expand_all(1.0);
        let vehicle_id = entity
            .vehicle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|vehicle| vehicle.get_entity().entity_id);
        let mut best: Option<(f64, Arc<dyn EntityBase>)> = None;
        for other in world.get_all_at_box(&search) {
            let candidate = other.get_entity();
            if candidate.entity_id == entity.entity_id
                || !selectable_entity(other.as_ref())
                || Some(candidate.entity_id) == vehicle_id
            {
                continue;
            }
            let Some((t, _)) = ray_box_hit(start, delta, &candidate.bounding_box.load()) else {
                continue;
            };
            if t * reach <= self.entity_interaction_range()
                && hit_is_before_block(t, block_t)
                && best.as_ref().is_none_or(|(nearest, _)| t < *nearest)
            {
                best = Some((t, other));
            }
        }
        if let Some((t, target)) = best {
            let relative_hit = start + delta * t - target.get_entity().pos.load();
            LookTarget::Entity(target, relative_hit)
        } else if let Some((t, hit)) = block_hit
            && t * reach <= self.block_interaction_range()
        {
            LookTarget::Block(hit)
        } else {
            LookTarget::Miss
        }
    }

    fn can_reach_entity(&self, target: &dyn EntityBase) -> bool {
        let entity = target.get_entity();
        if entity.entity_id == self.entity_id()
            || !selectable_entity(target)
            || !Arc::ptr_eq(&self.world(), &entity.world.load_full())
        {
            return false;
        }
        // This is a distance check, not a second raycast to the box's nearest
        // point. That point can be hidden while the actual crosshair hit above
        // or beside a partial wall is visible. Local look_target already tests
        // occlusion along the real hit ray; Java retains packet-target semantics.
        entity_within_reach(
            self.eye_position(),
            &entity.bounding_box.load(),
            self.entity_interaction_range(),
        )
    }

    pub(crate) fn is_using_item(&self) -> bool {
        self.using_item.load(Ordering::Relaxed)
            || self
                .living_entity
                .item_in_use
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
    }

    pub(crate) fn cancel_item_use(&self) {
        if self.is_using_item() {
            self.living_entity.clear_active_hand();
            self.stop_using_item();
        }
    }

    /// Release through the same registry hook used for a real player's release
    /// action. Death/spectator cancellation clears state without firing a shot.
    pub(crate) fn release_item_use(&self) {
        let stack = self
            .living_entity
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let hand = self
            .living_entity
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .or_else(|| self.using_hand.load());
        if self.can_perform_actions()
            && let Some(stack) = stack
            && let Some(hand) = hand
            && let Some(server) = self.world().server.upgrade()
        {
            server.item_registry.on_stopped_using(&stack, self, hand);
        }
        self.cancel_item_use();
    }

    fn stack_on_cooldown(&self, stack: &ItemStack) -> bool {
        let group = stack
            .get_use_cooldown()
            .and_then(|cooldown| cooldown.cooldown_group.as_deref())
            .unwrap_or(stack.item.registry_key);
        self.is_on_cooldown(group)
    }

    pub(crate) fn use_look_target(self: &Arc<Self>) -> bool {
        if !self.can_perform_actions() {
            return false;
        }
        if self.is_using_item() {
            return true;
        }
        let Some(server) = self.world().server.upgrade() else {
            return false;
        };
        let target = self.look_target();
        use_in_hand_order(
            |hand| match &target {
                LookTarget::Entity(entity, hit) => self.interact_entity(
                    entity.clone(),
                    ActionType::InteractAt,
                    hand,
                    Some(*hit),
                    &server,
                ),
                LookTarget::Block(hit) => self
                    .use_item_on(hit, hand, &server)
                    .unwrap_or(InteractionResult::Blocked),
                LookTarget::Miss => InteractionResult::Pass,
            },
            |hand| {
                self.use_item(
                    hand,
                    self.get_entity().yaw.load(),
                    self.get_entity().pitch.load(),
                    &server,
                )
            },
        )
        .consumes_action()
    }

    /// Entity callbacks, item callbacks, plugin cancellation, inventory updates,
    /// and `PvP` policy are common to Java and headless players.
    pub(crate) fn interact_entity(
        self: &Arc<Self>,
        target: Arc<dyn EntityBase>,
        action: ActionType,
        hand: Hand,
        target_position: Option<Vector3<f64>>,
        server: &Arc<Server>,
    ) -> InteractionResult {
        if !self.can_perform_actions() || !self.can_reach_entity(target.as_ref()) {
            return InteractionResult::Blocked;
        }
        self.update_last_action_time();
        let mut event = PlayerInteractEntityEvent::new(
            self,
            target,
            action,
            target_position,
            self.get_entity().is_sneaking(),
        );
        server.plugin_manager.fire_blocking(server, &mut event);
        if event.cancelled || !self.can_reach_entity(event.target.as_ref()) {
            return InteractionResult::Blocked;
        }
        if event.action == ActionType::Attack {
            if let Some(victim) = event.target.get_player() {
                let config = &server.advanced_config.pvp;
                if !config.enabled {
                    return InteractionResult::Blocked;
                }
                if config.protect_creative && victim.gamemode.load() == GameMode::Creative {
                    self.world().play_sound(
                        Sound::EntityPlayerAttackNodamage,
                        SoundCategory::Players,
                        &victim.position(),
                    );
                    return InteractionResult::Blocked;
                }
            }
            self.attack(&event.target);
            return InteractionResult::Consumed;
        }
        if event.action == ActionType::InteractAt
            && let Some(pos) = event.target_position
        {
            if !pos.x.is_finite() || !pos.y.is_finite() || !pos.z.is_finite() {
                return InteractionResult::Blocked;
            }
            let mut at_event = PlayerInteractAtEntityEvent::new(
                self.clone(),
                event.target.get_entity().entity_id,
                pos.x,
                pos.y,
                pos.z,
                u8::from(hand == Hand::Left),
            );
            server.plugin_manager.fire_blocking(server, &mut at_event);
            if at_event.cancelled {
                return InteractionResult::Blocked;
            }
        }
        let inventory = self.inventory();
        let mut stack = inventory.get_stack_in_hand(hand);
        let before = stack.clone();
        if self.stack_on_cooldown(&stack) {
            return InteractionResult::Pass;
        }
        let target_entity = event.target.get_entity();
        if target_entity.entity_type.resource_name == "zombie_villager"
            && stack.item.registry_key == "golden_apple"
        {
            self.trigger_advancement(
                super::advancement::trigger::AdvancementTrigger::CuredZombieVillager,
            );
        }
        let interacted = event.target.interact(self, &mut stack);
        let used = interacted
            || server
                .item_registry
                .use_on_entity(&mut stack, self, hand, event.target);
        if !stack.are_equal(&before) {
            self.sync_used_hand(hand, &before, stack);
        }
        InteractionResult::from_used(used)
    }

    /// Block and item-on-block use, including the normal placement/event path.
    #[expect(clippy::too_many_lines)]
    pub(crate) fn use_item_on(
        self: &Arc<Self>,
        hit: &BlockUseContext,
        hand: Hand,
        server: &Arc<Server>,
    ) -> Result<InteractionResult, BlockPlacingError> {
        if !self.can_perform_actions() {
            return Ok(InteractionResult::Blocked);
        }
        self.update_last_action_time();
        if !self.can_interact_with_block_at(&hit.position, 1.0)
            || ![hit.cursor_pos.x, hit.cursor_pos.y, hit.cursor_pos.z]
                .into_iter()
                .all(|coordinate| {
                    coordinate.is_finite() && (-1.0e-5..=1.000_01).contains(&coordinate)
                })
        {
            return Err(BlockPlacingError::BlockOutOfReach);
        }
        let world = self.world();
        let point = hit.position.0.to_f64()
            + Vector3::new(
                f64::from(hit.cursor_pos.x),
                f64::from(hit.cursor_pos.y),
                f64::from(hit.cursor_pos.z),
            );
        if block_hit_between(&world, self.eye_position(), point).is_some_and(|(t, obstruction)| {
            t < 1.0 - 1.0e-7 && obstruction.position != hit.position
        }) {
            return Err(BlockPlacingError::BlockOutOfReach);
        }
        let block = world.get_block(&hit.position);
        let mut event = PlayerInteractEvent::new(
            self,
            InteractAction::RightClickBlock,
            block,
            Some(hit.position),
        );
        server.plugin_manager.fire_blocking(server, &mut event);
        if event.cancelled {
            self.try_send_client_packet(&CBlockUpdate::new(
                hit.position,
                VarInt(i32::from(world.get_block_state_id(&hit.position).as_u16())),
            ));
            return Ok(InteractionResult::Blocked);
        }
        let inventory = self.inventory();
        let mut item = inventory.get_stack_in_hand(hand);
        let before = item.clone();
        let sneaking_with_item = self.get_entity().is_sneaking()
            && (!inventory.held_item().is_empty() || !inventory.off_hand_item().is_empty());
        if !sneaking_with_item {
            let slot = if hand == Hand::Right {
                EquipmentSlot::MAIN_HAND
            } else {
                EquipmentSlot::OFF_HAND
            };
            let result = server.block_registry.use_with_item(
                block,
                self,
                &hit.position,
                &BlockHitResult {
                    face: &hit.face,
                    cursor_pos: &hit.cursor_pos,
                },
                &mut item,
                &slot,
                server,
                &world,
            );
            let result = if matches!(result, BlockActionResult::PassToDefaultBlockAction) {
                server.block_registry.on_use(
                    block,
                    self,
                    &hit.position,
                    &BlockHitResult {
                        face: &hit.face,
                        cursor_pos: &hit.cursor_pos,
                    },
                    server,
                    &world,
                )
            } else {
                result
            };
            let outcome = InteractionResult::from_block_result(&result);
            if outcome == InteractionResult::Consumed {
                // Block handlers that use the mutable stack and handlers that
                // update inventory directly are both supported, without a
                // stale-stack write overwriting a direct handler update.
                if !item.are_equal(&before) {
                    self.sync_used_hand(hand, &before, item);
                }
                self.increment_stat(StatisticCategory::Used, i32::from(before.item.id), 1);
                if matches!(result, BlockActionResult::SuccessServer) {
                    self.swing_hand(hand, true);
                }
                return Ok(InteractionResult::Consumed);
            }
            if outcome == InteractionResult::Blocked {
                return Ok(outcome);
            }
        }
        if item.is_empty() || self.stack_on_cooldown(&item) {
            return Ok(InteractionResult::Pass);
        }
        let used = server.item_registry.use_on_block(
            &mut item,
            self,
            hand,
            hit.position,
            hit.face,
            hit.cursor_pos,
            block,
            server,
        );
        let placed = if !used && let Some(placed_block) = Block::from_item_id(item.item.id) {
            match server.block_registry.place_block(
                self,
                placed_block,
                server,
                hit,
                hit.position,
                hit.face,
            ) {
                Ok(Some((position, state))) => {
                    self.try_send_client_packet(&CBlockUpdate::new(
                        position,
                        VarInt(i32::from(state.as_u16())),
                    ));
                    item.decrement_unless_creative(self.gamemode.load(), 1);
                    true
                }
                // A failed/obstructed or plugin-vetoed placement is handled,
                // not a pass to air, offhand, or a simultaneous attack.
                Ok(None) => return Ok(InteractionResult::Blocked),
                Err(crate::block::registry::BlockPlacingError::InvalidGamemode) => {
                    return Err(BlockPlacingError::InvalidGamemode);
                }
                Err(crate::block::registry::BlockPlacingError::BlockOutOfWorld) => {
                    return Err(BlockPlacingError::BlockOutOfWorld);
                }
            }
        } else {
            false
        };
        if !item.are_equal(&before) {
            self.sync_used_hand(hand, &before, item);
        }
        if used || placed {
            self.increment_stat(StatisticCategory::Used, i32::from(before.item.id), 1);
            self.swing_hand(hand, true);
        }
        Ok(InteractionResult::from_used(used || placed))
    }

    pub(crate) fn sync_used_hand(&self, hand: Hand, before: &ItemStack, after: ItemStack) {
        let slot_index = if hand == Hand::Right {
            self.inventory.get_selected_slot() as usize
        } else {
            PlayerInventory::OFF_HAND_SLOT
        };
        if !before.is_empty() && after.is_empty() {
            let slot = if hand == Hand::Right {
                &EquipmentSlot::MAIN_HAND
            } else {
                &EquipmentSlot::OFF_HAND
            };
            self.world()
                .send_entity_status(self.get_entity(), equipment_break_status(slot), None);
        }
        self.sync_hand_slot(slot_index, after.clone());
        self.inventory.set_stack_in_hand(hand, after);
    }

    pub(crate) fn use_item(
        self: &Arc<Self>,
        hand: Hand,
        yaw: f32,
        pitch: f32,
        server: &Arc<Server>,
    ) -> InteractionResult {
        if !self.can_perform_actions() || !yaw.is_finite() || !pitch.is_finite() {
            return InteractionResult::Blocked;
        }
        if self.is_using_item() {
            return InteractionResult::Consumed;
        }
        let mut held = self.inventory().get_stack_in_hand(hand);
        if held.is_empty() || self.stack_on_cooldown(&held) {
            return InteractionResult::Pass;
        }
        self.update_last_action_time();
        let mut consume_event =
            PlayerItemConsumeEvent::new(self.clone(), held.item.registry_key.to_string());
        server
            .plugin_manager
            .fire_blocking(server, &mut consume_event);
        if consume_event.cancelled {
            return InteractionResult::Blocked;
        }
        let start = self.eye_position();
        let end = start
            + Vector3::rotation_vector(f64::from(pitch), f64::from(yaw))
                * self.block_interaction_range();
        let hit = block_hit_between(&self.world(), start, end);
        let mut event = if let Some((_, hit)) = hit {
            PlayerInteractEvent::new(
                self,
                InteractAction::RightClickBlock,
                self.world().get_block(&hit.position),
                Some(hit.position),
            )
        } else {
            PlayerInteractEvent::new(self, InteractAction::RightClickAir, &Block::AIR, None)
        };
        server.plugin_manager.fire_blocking(server, &mut event);
        if event.cancelled {
            return InteractionResult::Blocked;
        }
        if held.item.id == Item::FISHING_ROD.id {
            let mut fish_event = PlayerFishEvent::new(
                self.clone(),
                None,
                uuid::Uuid::nil(),
                String::new(),
                PlayerFishState::Fishing,
                hand,
                0,
            );
            server.plugin_manager.fire_blocking(server, &mut fish_event);
            if fish_event.cancelled {
                return InteractionResult::Blocked;
            }
        }
        let before = held.clone();
        let prepared = self.prepare_hand_item_for_use(hand, &mut held);
        let item_used = server.item_registry.on_use(&before, self, hand);
        if prepared || item_used {
            self.increment_stat(StatisticCategory::Used, i32::from(before.item.id), 1);
        }
        InteractionResult::from_used(prepared || item_used)
    }

    fn prepare_hand_item_for_use(&self, hand: Hand, held: &mut ItemStack) -> bool {
        let mut used = false;
        if held.get_data_component::<ConsumableImpl>().is_some()
            || held.get_data_component::<BlocksAttacksImpl>().is_some()
        {
            let can_consume = held.get_data_component::<FoodImpl>().is_none_or(|food| {
                self.abilities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .invulnerable
                    || food.can_always_eat
                    || self.hunger_manager.level.load() < 20
            });
            if can_consume {
                self.living_entity
                    .set_active_hand(hand, held.clone(), held.get_max_use_time());
                used = true;
            }
        }
        let equipment_slot = held
            .get_data_component::<EquippableImpl>()
            .map(|component| component.slot.clone());
        if let Some(slot) = equipment_slot {
            let inventory = self.inventory();
            let current = inventory
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&slot);
            if !current.are_items_and_components_equal(held) {
                self.enqueue_equipment_change(&slot, held);
                let equipped = if current.is_empty() {
                    let equipped = held.copy_with_count(1);
                    held.decrement_unless_creative(self.gamemode.load(), 1);
                    equipped
                } else {
                    std::mem::replace(held, current)
                };
                inventory
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .put(&slot, equipped);
                inventory.set_stack_in_hand(hand, held.clone());
                used = true;
            }
        }
        used
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn pending_removal_blocks_old_snapshots_before_removal_reason_is_set() {
        assert!(entity_lifecycle_allows_actions(false, true));
        // Player::remove is still cleaning chunks: is_alive() remains true,
        // but kill has already set the independent removal-request flag.
        assert!(!entity_lifecycle_allows_actions(true, true));
        assert!(!entity_lifecycle_allows_actions(false, false));
        assert!(!entity_lifecycle_allows_actions(true, false));
    }

    #[test]
    fn partially_hidden_entities_remain_reachable_along_visible_hit_rays() {
        for (eye, end, target, wall) in [
            (
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::new(3.0, 3.4, 0.0),
                BoundingBox::new(Vector3::new(2.0, 0.0, -0.5), Vector3::new(3.0, 3.0, 0.5)),
                BoundingBox::new(Vector3::new(1.0, 0.0, -1.0), Vector3::new(1.5, 1.5, 1.0)),
            ),
            (
                Vector3::new(0.0, 0.0, 1.0),
                Vector3::new(3.0, 0.0, 3.4),
                BoundingBox::new(Vector3::new(2.0, -0.5, 0.0), Vector3::new(3.0, 0.5, 3.0)),
                BoundingBox::new(Vector3::new(1.0, -1.0, 0.0), Vector3::new(1.5, 1.0, 1.5)),
            ),
        ] {
            let eye: Vector3<f64> = eye;
            let nearest = Vector3::new(
                eye.x.clamp(target.min.x, target.max.x),
                eye.y.clamp(target.min.y, target.max.y),
                eye.z.clamp(target.min.z, target.max.z),
            );
            assert!(ray_box_hit(eye, nearest - eye, &wall).is_some());
            let hit = ray_box_hit(eye, end - eye, &target).unwrap();
            let block_hit = ray_box_hit(eye, end - eye, &wall).map(|hit| hit.0);
            assert!(hit_is_before_block(hit.0, block_hit));
            assert!(entity_within_reach(eye, &target, 3.0));
            assert!(!entity_within_reach(eye, &target, 1.9));
        }
    }

    #[test]
    fn block_dda_starts_in_air_and_finds_nearest_solid() {
        let start = Vector3::new(0.5, 0.5, 0.5);
        let end = Vector3::new(5.5, 0.5, 0.5);
        let hit = trace_block_outlines(start, end, |position| {
            if position.0.x == 2 || position.0.x == 4 {
                vec![BoundingBox::from_block(&position)]
            } else {
                Vec::new()
            }
        });
        let Some((t, hit)) = hit else {
            panic!("ray missed solid after air");
        };
        assert!((t - 0.3).abs() < 1.0e-9);
        assert_eq!(hit.position, BlockPos::new(2, 0, 0));
        assert_eq!(hit.face, BlockDirection::West);
    }

    #[test]
    fn block_dda_passes_above_slab_and_handles_negative_direction() {
        for (start, end) in [
            (Vector3::new(0.5, 0.75, 0.5), Vector3::new(5.5, 0.75, 0.5)),
            (Vector3::new(5.5, 0.75, 0.5), Vector3::new(0.5, 0.75, 0.5)),
        ] {
            let solid_x = if start.x < end.x { 4 } else { 1 };
            let hit = trace_block_outlines(start, end, |position| {
                if position.0.x == solid_x {
                    vec![BoundingBox::from_block(&position)]
                } else if position.0.x == 2 || position.0.x == 3 {
                    vec![BoundingBox::new(
                        position.0.to_f64(),
                        position.0.to_f64().add_raw(1.0, 0.5, 1.0),
                    )]
                } else {
                    Vec::new()
                }
            });
            assert_eq!(
                hit.map(|(_, hit)| hit.position),
                Some(BlockPos::new(solid_x, 0, 0))
            );
        }
    }

    #[test]
    fn block_dda_does_not_select_beyond_segment() {
        let hit = trace_block_outlines(
            Vector3::new(0.5, 0.5, 0.5),
            Vector3::new(1.5, 0.5, 0.5),
            |position| {
                if position.0.x == 2 {
                    vec![BoundingBox::from_block(&position)]
                } else {
                    Vec::new()
                }
            },
        );
        assert!(hit.is_none());
    }

    #[test]
    fn cancelled_block_callbacks_block_air_and_offhand_but_normal_pass_does_not() {
        let calls = RefCell::new(Vec::new());
        let result = use_in_hand_order(
            |hand| {
                calls.borrow_mut().push(("block", hand));
                // CauldronLevelChange/BellRing vetoes return Fail, not Pass.
                InteractionResult::from_block_result(&BlockActionResult::Fail)
            },
            |hand| {
                calls.borrow_mut().push(("air", hand));
                InteractionResult::Consumed
            },
        );
        assert_eq!(result, InteractionResult::Blocked);
        assert_eq!(*calls.borrow(), vec![("block", Hand::Right)]);
        assert_eq!(
            InteractionResult::from_block_result(&BlockActionResult::Pass),
            InteractionResult::Pass
        );
        assert_eq!(
            InteractionResult::from_block_result(&BlockActionResult::PassToDefaultBlockAction),
            InteractionResult::Pass
        );
    }

    #[test]
    fn consumed_target_or_plugin_veto_never_uses_air_or_offhand() {
        for result in [InteractionResult::Consumed, InteractionResult::Blocked] {
            let called = RefCell::new(Vec::new());
            assert_eq!(
                use_in_hand_order(
                    |hand| {
                        called.borrow_mut().push(("target", hand));
                        result
                    },
                    |hand| {
                        called.borrow_mut().push(("air", hand));
                        InteractionResult::Consumed
                    },
                ),
                result
            );
            assert_eq!(*called.borrow(), vec![("target", Hand::Right)]);
        }
    }

    #[test]
    fn passing_target_uses_main_hand_air_before_offhand() {
        let called = RefCell::new(Vec::new());
        assert_eq!(
            use_in_hand_order(
                |hand| {
                    called.borrow_mut().push(("target", hand));
                    InteractionResult::Pass
                },
                |hand| {
                    called.borrow_mut().push(("air", hand));
                    InteractionResult::from_used(hand == Hand::Left)
                },
            ),
            InteractionResult::Consumed
        );
        assert_eq!(
            *called.borrow(),
            vec![
                ("target", Hand::Right),
                ("air", Hand::Right),
                ("target", Hand::Left),
                ("air", Hand::Left)
            ]
        );
    }

    #[test]
    fn outline_ray_selects_actual_face_and_respects_occlusion_ties() {
        let start = Vector3::new(0.0, 0.25, 0.5);
        let delta = Vector3::new(4.0, 0.0, 0.0);
        let slab = BoundingBox::new(Vector3::new(2.0, 0.0, 0.0), Vector3::new(3.0, 0.5, 1.0));
        assert_eq!(
            ray_box_hit(start, delta, &slab),
            Some((0.5, BlockDirection::West))
        );
        assert_eq!(
            ray_box_hit(start.add_raw(0.0, 0.5, 0.0), delta, &slab),
            None
        );
        assert!(hit_is_before_block(0.4, Some(0.5)));
        assert!(!hit_is_before_block(0.5, Some(0.5)));
        assert!(!hit_is_before_block(0.6, Some(0.5)));
        assert!(hit_is_before_block(0.9, None));
    }

    #[test]
    fn ray_rejects_out_of_range_and_non_finite_hits() {
        let bounding_box =
            BoundingBox::new(Vector3::new(2.0, 0.0, 0.0), Vector3::new(3.0, 1.0, 1.0));
        assert_eq!(
            ray_box_hit(
                Vector3::new(0.0, 0.5, 0.5),
                Vector3::new(1.0, 0.0, 0.0),
                &bounding_box
            ),
            None
        );
        assert_eq!(
            ray_box_hit(
                Vector3::new(f64::NAN, 0.5, 0.5),
                Vector3::new(4.0, 0.0, 0.0),
                &bounding_box
            ),
            None
        );
    }
}
