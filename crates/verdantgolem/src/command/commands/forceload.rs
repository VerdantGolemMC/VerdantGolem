use crate::world::World;

use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::coordinates::Coordinates;
use crate::command::argument_types::coordinates::column_pos::ColumnPosArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::errors::command_syntax_error::CommandSyntaxError;
use crate::command::errors::error_types::{CommandErrorType, LiteralCommandErrorType};
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
    translation::java::COMMANDS_FORCELOAD_ADDED_FAILURE,
);
static ERROR_FAILED_REMOVE: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_FORCELOAD_REMOVED_FAILURE,
    translation::java::COMMANDS_FORCELOAD_REMOVED_FAILURE,
);
static ERROR_FAILED_QUERY: CommandErrorType<2> = CommandErrorType::new(
    translation::java::COMMANDS_FORCELOAD_QUERY_FAILURE,
    translation::java::COMMANDS_FORCELOAD_QUERY_FAILURE,
);
static ERROR_TOO_MANY: CommandErrorType<2> = CommandErrorType::new(
    translation::java::COMMANDS_FORCELOAD_TOOBIG,
    translation::java::COMMANDS_FORCELOAD_TOOBIG,
);
static ERROR_NO_WORLD: LiteralCommandErrorType =
    LiteralCommandErrorType::new("Forceload requires a world context");
static ERROR_STORAGE: LiteralCommandErrorType =
    LiteralCommandErrorType::new("Forced chunk storage error");

fn storage_error(world: &World, error: &std::io::Error) -> CommandSyntaxError {
    tracing::error!(
        dimension = world.dimension.minecraft_name,
        %error,
        "Forceload command failed"
    );
    CommandSyntaxError::create_without_context(
        &ERROR_STORAGE,
        TextComponent::text(format!("Forced chunk storage error: {error}")),
    )
}

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

fn chunk_text(position: Vector2<i32>) -> TextComponent {
    TextComponent::text(format!("[{}, {}]", position.x, position.y))
}

#[derive(Clone, Copy)]
enum ChangeKind {
    Add,
    Remove,
}

fn change_feedback(
    change: ChangeKind,
    count: u64,
    from: Vector2<i32>,
    to: Vector2<i32>,
    dimension: &str,
) -> TextComponent {
    let (single, multiple) = match change {
        ChangeKind::Add => (
            translation::java::COMMANDS_FORCELOAD_ADDED_SINGLE,
            translation::java::COMMANDS_FORCELOAD_ADDED_MULTIPLE,
        ),
        ChangeKind::Remove => (
            translation::java::COMMANDS_FORCELOAD_REMOVED_SINGLE,
            translation::java::COMMANDS_FORCELOAD_REMOVED_MULTIPLE,
        ),
    };
    let dimension = TextComponent::text(dimension.to_owned());
    // A multi-chunk request may change only one previously unmarked/marked
    // chunk. Report its requested bounds rather than falsely naming `from`
    // as that single changed chunk; the store deliberately returns a count.
    if from == to {
        TextComponent::translate_cross(single, single, [chunk_text(from), dimension])
    } else {
        TextComponent::translate_cross(
            multiple,
            multiple,
            [
                TextComponent::text(count.to_string()),
                dimension,
                chunk_text(from),
                chunk_text(to),
            ],
        )
    }
}

fn remove_all_feedback(dimension: &str) -> TextComponent {
    TextComponent::translate_cross(
        translation::java::COMMANDS_FORCELOAD_REMOVED_ALL,
        translation::java::COMMANDS_FORCELOAD_REMOVED_ALL,
        [TextComponent::text(dimension.to_owned())],
    )
}

fn query_feedback(
    forced: &[Vector2<i32>],
    position: Option<Vector2<i32>>,
    dimension: &str,
) -> Result<(TextComponent, i32), CommandSyntaxError> {
    let dimension = TextComponent::text(dimension.to_owned());
    if let Some(position) = position {
        if !forced.contains(&position) {
            // Returning Err is important: Ok(0) still reports success=1 to
            // `/execute store success`, despite the chunk not being forced.
            return Err(ERROR_FAILED_QUERY.create_without_context(chunk_text(position), dimension));
        }
        return Ok((
            TextComponent::translate_cross(
                translation::java::COMMANDS_FORCELOAD_QUERY_SUCCESS,
                translation::java::COMMANDS_FORCELOAD_QUERY_SUCCESS,
                [chunk_text(position), dimension],
            )
            .color_named(NamedColor::Green),
            1,
        ));
    }

    let list = TextComponent::text(
        forced
            .iter()
            .map(|position| format!("[{}, {}]", position.x, position.y))
            .collect::<Vec<_>>()
            .join(", "),
    );
    let text = match forced.len() {
        0 => TextComponent::translate_cross(
            translation::java::COMMANDS_FORCELOAD_ADDED_NONE,
            translation::java::COMMANDS_FORCELOAD_ADDED_NONE,
            [dimension],
        ),
        1 => TextComponent::translate_cross(
            translation::java::COMMANDS_FORCELOAD_LIST_SINGLE,
            translation::java::COMMANDS_FORCELOAD_LIST_SINGLE,
            [dimension, list],
        ),
        count => TextComponent::translate_cross(
            translation::java::COMMANDS_FORCELOAD_LIST_MULTIPLE,
            translation::java::COMMANDS_FORCELOAD_LIST_MULTIPLE,
            [TextComponent::text(count.to_string()), dimension, list],
        ),
    };
    // Listing is a successful query, even when its numeric result is zero.
    Ok((
        text.color_named(NamedColor::Gray),
        command_count(forced.len() as u64),
    ))
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
            .ok_or_else(|| ERROR_NO_WORLD.create_without_context())?;

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

        let changed_chunks = world
            .add_forced_chunks(
                (min_x..=max_x).flat_map(|x| (min_z..=max_z).map(move |z| Vector2::new(x, z))),
            )
            .map_err(|error| storage_error(world, &error))? as u64;
        if changed_chunks == 0 {
            return Err(ERROR_FAILED_ADD.create_without_context());
        }

        let text = change_feedback(
            ChangeKind::Add,
            changed_chunks,
            Vector2::new(min_x, min_z),
            Vector2::new(max_x, max_z),
            world.dimension.minecraft_name,
        );
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
            .ok_or_else(|| ERROR_NO_WORLD.create_without_context())?;

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

        let changed_chunks = world
            .remove_forced_chunks(
                (min_x..=max_x).flat_map(|x| (min_z..=max_z).map(move |z| Vector2::new(x, z))),
            )
            .map_err(|error| storage_error(world, &error))? as u64;
        if changed_chunks == 0 {
            return Err(ERROR_FAILED_REMOVE.create_without_context());
        }

        let text = change_feedback(
            ChangeKind::Remove,
            changed_chunks,
            Vector2::new(min_x, min_z),
            Vector2::new(max_x, max_z),
            world.dimension.minecraft_name,
        );
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
            .ok_or_else(|| ERROR_NO_WORLD.create_without_context())?;

        let removed = world
            .clear_forced_chunks()
            .map_err(|error| storage_error(world, &error))?;

        let text = remove_all_feedback(world.dimension.minecraft_name);
        context
            .source
            .send_feedback(text.color(Color::Named(NamedColor::Green)), false);

        Ok(i32::try_from(removed).unwrap_or(i32::MAX))
    }
}

struct ForceloadQueryExecutor;

impl CommandExecutor for ForceloadQueryExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let world = context
            .source
            .world
            .as_ref()
            .ok_or_else(|| ERROR_NO_WORLD.create_without_context())?;

        let chunk_pos = if context.get_argument::<Coordinates>("pos").is_ok() {
            let pos = ColumnPosArgumentType::get_column_pos(context, "pos")?;
            Some(Vector2::new(pos.0.x >> 4, pos.0.y >> 4))
        } else {
            None
        };

        let forced = world
            .forced_chunk_positions()
            .map_err(|error| storage_error(world, &error))?;
        let (text, result) = query_feedback(&forced, chunk_pos, world.dimension.minecraft_name)?;
        context.source.send_feedback(text, false);

        Ok(result)
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
    use super::*;

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

    #[test]
    fn add_remove_feedback_matches_the_actual_translation_arguments() {
        let from = Vector2::new(-2, 3);
        let to = Vector2::new(1, 5);
        for (change, single, multiple) in [
            (
                ChangeKind::Add,
                "Marked chunk [-2, 3] in minecraft:the_nether to be force loaded",
                "Marked 12 chunks in minecraft:the_nether from [-2, 3] to [1, 5] to be force loaded",
            ),
            (
                ChangeKind::Remove,
                "Unmarked chunk [-2, 3] in minecraft:the_nether for force loading",
                "Unmarked 12 chunks in minecraft:the_nether from [-2, 3] to [1, 5] for force loading",
            ),
        ] {
            assert_eq!(
                change_feedback(change, 1, from, from, "minecraft:the_nether").get_text(),
                single
            );
            assert_eq!(
                change_feedback(change, 12, from, to, "minecraft:the_nether").get_text(),
                multiple
            );
        }
        assert_eq!(
            remove_all_feedback("minecraft:the_end").get_text(),
            "Unmarked all force loaded chunks in minecraft:the_end"
        );
    }

    #[test]
    fn partially_changed_ranges_do_not_claim_the_first_chunk_changed() {
        let text = change_feedback(
            ChangeKind::Add,
            1,
            Vector2::new(0, 0),
            Vector2::new(1, 0),
            "minecraft:overworld",
        )
        .get_text();
        assert_eq!(
            text,
            "Marked 1 chunks in minecraft:overworld from [0, 0] to [1, 0] to be force loaded"
        );
    }

    #[test]
    fn query_without_position_lists_all_chunks_and_returns_the_count() {
        let forced = [Vector2::new(-5, 2), Vector2::new(8, -3)];
        let (text, count) = query_feedback(&forced, None, "minecraft:overworld").unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            text.get_text(),
            "2 force loaded chunks were found in minecraft:overworld at: [-5, 2], [8, -3]"
        );

        let (text, count) = query_feedback(&forced[..1], None, "minecraft:the_end").unwrap();
        assert_eq!(count, 1);
        assert_eq!(
            text.get_text(),
            "A force loaded chunk was found in minecraft:the_end at: [-5, 2]"
        );
    }

    #[test]
    fn empty_list_query_succeeds_with_zero_instead_of_querying_the_sender_chunk() {
        // An empty *list* is still a successful query: execute store result
        // gets 0, while execute store success gets 1.
        let (text, count) = query_feedback(&[], None, "minecraft:overworld").unwrap();
        assert_eq!(count, 0);
        assert_eq!(
            text.get_text(),
            "No force loaded chunks were found in minecraft:overworld"
        );
    }

    #[test]
    fn positioned_query_returns_one_or_a_real_command_failure() {
        let position = Vector2::new(10, 10);
        let (text, count) =
            query_feedback(&[position], Some(position), "minecraft:overworld").unwrap();
        assert_eq!(count, 1);
        assert_eq!(
            text.get_text(),
            "Chunk at [10, 10] in minecraft:overworld is marked for force loading"
        );

        // The production executor propagates this Err with `?`; the command
        // dispatcher therefore records ReturnValue::Failure, not Success(0).
        let error = query_feedback(&[], Some(position), "minecraft:overworld").unwrap_err();
        assert!(error.is(&ERROR_FAILED_QUERY));
        assert_eq!(
            error.message.get_text(),
            "Chunk at [10, 10] in minecraft:overworld is not marked for force loading"
        );
    }
}
