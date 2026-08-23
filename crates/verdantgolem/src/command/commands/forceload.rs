use std::sync::Arc;

use crate::world::World;

use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::coordinates::Coordinates;
use crate::command::argument_types::coordinates::column_pos::ColumnPosArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use verdantgolem_data::translation;
use verdantgolem_util::PermissionLvl;
use verdantgolem_util::math::vector2::Vector2;
use verdantgolem_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use verdantgolem_util::text::TextComponent;
use verdantgolem_util::text::color::{Color, NamedColor};

const DESCRIPTION: &str = "Constantly load chunks in the world.";
const PERMISSION: &str = "minecraft:command.forceload";

static ERROR_FAILED_ADD: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_FORCELOAD_ADDED_FAILURE,
    "No world in source",
);
static ERROR_FAILED_REMOVE: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_FORCELOAD_REMOVED_FAILURE,
    "No world in source",
);
static ERROR_FAILED_QUERY: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_FORCELOAD_QUERY_FAILURE,
    "No world in source",
);
static ERROR_TOO_MANY: CommandErrorType<2> = CommandErrorType::new(
    translation::java::COMMANDS_FORCELOAD_TOOBIG,
    "Too many chunks",
);

fn inclusive_chunk_area(min_x: i32, max_x: i32, min_z: i32, max_z: i32) -> Option<u64> {
    let count_x = i64::from(max_x)
        .checked_sub(i64::from(min_x))?
        .checked_add(1)?;
    let count_z = i64::from(max_z)
        .checked_sub(i64::from(min_z))?
        .checked_add(1)?;
    let count_x = u64::try_from(count_x).ok()?;
    let count_z = u64::try_from(count_z).ok()?;
    count_x.checked_mul(count_z)
}

fn command_count(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

struct ForceloadAddExecutor;

impl CommandExecutor for ForceloadAddExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let from_pos = ColumnPosArgumentType::get_column_pos(context, "from")?;

        let to_pos = if context.get_argument::<Coordinates>("to").is_ok() {
            ColumnPosArgumentType::get_column_pos(context, "to")?
        } else {
            from_pos
        };

        let world = context
            .source
            .world
            .as_ref()
            .ok_or_else(|| ERROR_FAILED_ADD.create_without_context())?;

        let chunk_x_start = from_pos.0.x >> 4;
        let chunk_z_start = from_pos.0.y >> 4;

        let chunk_x_end = to_pos.0.x >> 4;
        let chunk_z_end = to_pos.0.y >> 4;

        let min_x = chunk_x_start.min(chunk_x_end);
        let max_x = chunk_x_start.max(chunk_x_end);
        let min_z = chunk_z_start.min(chunk_z_end);
        let max_z = chunk_z_start.max(chunk_z_end);

        let total_chunks = inclusive_chunk_area(min_x, max_x, min_z, max_z).unwrap_or(u64::MAX);

        let forceload_limit = crate::carpet::values()
            .forceload_limit
            .clamp(1, i64::from(i32::MAX)) as u64;
        if total_chunks > forceload_limit {
            return Err(ERROR_TOO_MANY.create_without_context(
                TextComponent::text(forceload_limit.to_string()),
                TextComponent::text(total_chunks.to_string()),
            ));
        }

        let added = {
            let mut forced = world
                .forced_chunks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut added = Vec::new();
            for x in min_x..=max_x {
                for z in min_z..=max_z {
                    let position = Vector2::new(x, z);
                    if forced.insert(position) {
                        added.push(position);
                    }
                }
            }
            added
        };
        if added.is_empty() {
            return Err(ERROR_FAILED_ADD.create_without_context());
        }
        add_force_tickets(world, &added);

        world.update_active_chunks();
        let changed_chunks = added.len() as u64;

        let text = if changed_chunks == 1 {
            TextComponent::translate_cross(
                translation::java::COMMANDS_FORCELOAD_ADDED_SINGLE,
                translation::java::COMMANDS_FORCELOAD_ADDED_SINGLE,
                [
                    TextComponent::text(min_x.to_string()),
                    TextComponent::text(min_z.to_string()),
                ],
            )
        } else {
            TextComponent::translate_cross(
                translation::java::COMMANDS_FORCELOAD_ADDED_MULTIPLE,
                translation::java::COMMANDS_FORCELOAD_ADDED_MULTIPLE,
                [
                    TextComponent::text(changed_chunks.to_string()),
                    TextComponent::text(min_x.to_string()),
                    TextComponent::text(min_z.to_string()),
                    TextComponent::text(max_x.to_string()),
                    TextComponent::text(max_z.to_string()),
                ],
            )
        };
        context
            .source
            .send_feedback(text.color(Color::Named(NamedColor::Green)), false);

        Ok(command_count(changed_chunks))
    }
}

struct ForceloadRemoveExecutor;

impl CommandExecutor for ForceloadRemoveExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let from_pos = ColumnPosArgumentType::get_column_pos(context, "from")?;

        let to_pos = if context.get_argument::<Coordinates>("to").is_ok() {
            ColumnPosArgumentType::get_column_pos(context, "to")?
        } else {
            from_pos
        };

        let world = context
            .source
            .world
            .as_ref()
            .ok_or_else(|| ERROR_FAILED_REMOVE.create_without_context())?;

        let chunk_x_start = from_pos.0.x >> 4;
        let chunk_z_start = from_pos.0.y >> 4;

        let chunk_x_end = to_pos.0.x >> 4;
        let chunk_z_end = to_pos.0.y >> 4;

        let min_x = chunk_x_start.min(chunk_x_end);
        let max_x = chunk_x_start.max(chunk_x_end);
        let min_z = chunk_z_start.min(chunk_z_end);
        let max_z = chunk_z_start.max(chunk_z_end);

        let total_chunks = inclusive_chunk_area(min_x, max_x, min_z, max_z).unwrap_or(u64::MAX);

        let forceload_limit = crate::carpet::values()
            .forceload_limit
            .clamp(1, i64::from(i32::MAX)) as u64;
        if total_chunks > forceload_limit {
            return Err(ERROR_TOO_MANY.create_without_context(
                TextComponent::text(forceload_limit.to_string()),
                TextComponent::text(total_chunks.to_string()),
            ));
        }

        let removed = {
            let mut forced = world
                .forced_chunks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut removed = Vec::new();
            for x in min_x..=max_x {
                for z in min_z..=max_z {
                    let position = Vector2::new(x, z);
                    if forced.remove(&position) {
                        removed.push(position);
                    }
                }
            }
            removed
        };
        if removed.is_empty() {
            return Err(ERROR_FAILED_REMOVE.create_without_context());
        }
        remove_force_tickets(world, &removed);

        world.update_active_chunks();
        let changed_chunks = removed.len() as u64;

        let text = if changed_chunks == 1 {
            TextComponent::translate_cross(
                translation::java::COMMANDS_FORCELOAD_REMOVED_SINGLE,
                translation::java::COMMANDS_FORCELOAD_REMOVED_SINGLE,
                [
                    TextComponent::text(min_x.to_string()),
                    TextComponent::text(min_z.to_string()),
                ],
            )
        } else {
            TextComponent::translate_cross(
                translation::java::COMMANDS_FORCELOAD_REMOVED_MULTIPLE,
                translation::java::COMMANDS_FORCELOAD_REMOVED_MULTIPLE,
                [
                    TextComponent::text(changed_chunks.to_string()),
                    TextComponent::text(min_x.to_string()),
                    TextComponent::text(min_z.to_string()),
                    TextComponent::text(max_x.to_string()),
                    TextComponent::text(max_z.to_string()),
                ],
            )
        };
        context
            .source
            .send_feedback(text.color(Color::Named(NamedColor::Green)), false);

        Ok(command_count(changed_chunks))
    }
}

struct ForceloadRemoveAllExecutor;

impl CommandExecutor for ForceloadRemoveAllExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let world = context
            .source
            .world
            .as_ref()
            .ok_or_else(|| ERROR_FAILED_REMOVE.create_without_context())?;

        let removed = {
            let mut forced = world
                .forced_chunks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            forced.drain().collect::<Vec<_>>()
        };
        remove_force_tickets(world, &removed);

        world.update_active_chunks();

        let text = TextComponent::translate_cross(
            translation::java::COMMANDS_FORCELOAD_REMOVED_ALL,
            translation::java::COMMANDS_FORCELOAD_REMOVED_ALL,
            [],
        );
        context
            .source
            .send_feedback(text.color(Color::Named(NamedColor::Green)), false);

        Ok(i32::try_from(removed.len()).unwrap_or(i32::MAX))
    }
}

fn add_force_tickets(world: &Arc<World>, positions: &[Vector2<i32>]) {
    let mut loading = world
        .level
        .chunk_loading
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for position in positions {
        loading.add_force_ticket(*position);
    }
    loading.send_change();
}

fn remove_force_tickets(world: &Arc<World>, positions: &[Vector2<i32>]) {
    let mut loading = world
        .level
        .chunk_loading
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for position in positions {
        loading.remove_force_ticket(*position);
    }
    loading.send_change();
}

struct ForceloadQueryExecutor;

impl CommandExecutor for ForceloadQueryExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let world = context
            .source
            .world
            .as_ref()
            .ok_or_else(|| ERROR_FAILED_QUERY.create_without_context())?;

        let chunk_pos = if context.get_argument::<Coordinates>("pos").is_ok() {
            let pos = ColumnPosArgumentType::get_column_pos(context, "pos")?;
            Vector2::new(pos.0.x >> 4, pos.0.y >> 4)
        } else {
            let block_x = context.source.position.x.floor() as i32;
            let block_z = context.source.position.z.floor() as i32;
            Vector2::new(block_x >> 4, block_z >> 4)
        };

        let is_forced = {
            let forced = world
                .forced_chunks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            forced.contains(&chunk_pos)
        };

        if is_forced {
            let text = TextComponent::translate_cross(
                translation::java::COMMANDS_FORCELOAD_QUERY_SUCCESS,
                translation::java::COMMANDS_FORCELOAD_QUERY_SUCCESS,
                [
                    TextComponent::text(chunk_pos.x.to_string()),
                    TextComponent::text(chunk_pos.y.to_string()),
                ],
            );
            context
                .source
                .send_feedback(text.color(Color::Named(NamedColor::Green)), false);
        } else {
            let text = TextComponent::translate_cross(
                translation::java::COMMANDS_FORCELOAD_QUERY_FAILURE,
                translation::java::COMMANDS_FORCELOAD_QUERY_FAILURE,
                [
                    TextComponent::text(chunk_pos.x.to_string()),
                    TextComponent::text(chunk_pos.y.to_string()),
                ],
            );
            context
                .source
                .send_feedback(text.color(Color::Named(NamedColor::Red)), false);
        }

        let all_forced = {
            let forced = world
                .forced_chunks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            forced
                .iter()
                .map(|pos| format!("[{}, {}]", pos.x, pos.y))
                .collect::<Vec<_>>()
        };

        if all_forced.is_empty() {
            let text = TextComponent::translate_cross(
                translation::java::COMMANDS_FORCELOAD_ADDED_NONE,
                translation::java::COMMANDS_FORCELOAD_ADDED_NONE,
                [],
            );
            context
                .source
                .send_feedback(text.color(Color::Named(NamedColor::Gray)), false);
        } else {
            let list_str = all_forced.join(", ");
            let text = if all_forced.len() == 1 {
                TextComponent::translate_cross(
                    translation::java::COMMANDS_FORCELOAD_LIST_SINGLE,
                    translation::java::COMMANDS_FORCELOAD_LIST_SINGLE,
                    [TextComponent::text(list_str)],
                )
            } else {
                TextComponent::translate_cross(
                    translation::java::COMMANDS_FORCELOAD_LIST_MULTIPLE,
                    translation::java::COMMANDS_FORCELOAD_LIST_MULTIPLE,
                    [TextComponent::text(list_str)],
                )
            };
            context
                .source
                .send_feedback(text.color(Color::Named(NamedColor::Gray)), false);
        }

        Ok(i32::from(is_forced))
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    let builder = command("forceload", DESCRIPTION)
        .requires(PERMISSION)
        .then(
            literal("add").then(
                argument("from", ColumnPosArgumentType)
                    .executes(ForceloadAddExecutor)
                    .then(argument("to", ColumnPosArgumentType).executes(ForceloadAddExecutor)),
            ),
        )
        .then(
            literal("remove")
                .then(literal("all").executes(ForceloadRemoveAllExecutor))
                .then(
                    argument("from", ColumnPosArgumentType)
                        .executes(ForceloadRemoveExecutor)
                        .then(
                            argument("to", ColumnPosArgumentType).executes(ForceloadRemoveExecutor),
                        ),
                ),
        )
        .then(
            literal("query")
                .executes(ForceloadQueryExecutor)
                .then(argument("pos", ColumnPosArgumentType).executes(ForceloadQueryExecutor)),
        );

    dispatcher.register(builder);
}

#[cfg(test)]
mod tests {
    use super::{command_count, inclusive_chunk_area};

    #[test]
    fn chunk_area_uses_checked_wide_arithmetic() {
        assert_eq!(inclusive_chunk_area(-2, 2, -3, 3), Some(35));
        assert_eq!(
            inclusive_chunk_area(i32::MIN, i32::MAX, 0, 0),
            Some(u64::from(u32::MAX) + 1)
        );
        assert_eq!(
            inclusive_chunk_area(i32::MIN, i32::MAX, i32::MIN, i32::MAX),
            None
        );
    }

    #[test]
    fn command_result_saturates() {
        assert_eq!(command_count(u64::MAX), i32::MAX);
    }
}
