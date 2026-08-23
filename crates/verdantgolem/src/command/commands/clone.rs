use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::block::BlockArgumentType;
use crate::command::argument_types::coordinates::block_pos::BlockPosArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use verdantgolem_data::{Block, BlockStateId, translation};
use verdantgolem_nbt::compound::NbtCompound;
use verdantgolem_util::PermissionLvl;
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_util::math::vector2::Vector2;
use verdantgolem_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use verdantgolem_util::text::TextComponent;
use verdantgolem_world::world::BlockFlags;

const DESCRIPTION: &str = "Clones blocks from one region to another.";
const PERMISSION: &str = "minecraft:command.clone";

const ARG_BEGIN: &str = "begin";
const ARG_END: &str = "end";
const ARG_DEST: &str = "destination";
const ARG_FILTER: &str = "filter";

const OVERLAP_ERROR: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_CLONE_OVERLAP,
    translation::bedrock::COMMANDS_CLONE_NOOVERLAP,
);

const TOOBIG_ERROR: CommandErrorType<2> = CommandErrorType::new(
    translation::java::COMMANDS_CLONE_TOOBIG,
    translation::bedrock::COMMANDS_CLONE_TOOMANYBLOCKS,
);

const FAILED_ERROR: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_CLONE_FAILED,
    translation::bedrock::COMMANDS_CLONE_FAILED,
);

const NOT_LOADED_ERROR: CommandErrorType<0> = CommandErrorType::new(
    translation::java::ARGUMENT_POS_UNLOADED,
    translation::java::ARGUMENT_POS_UNLOADED,
);

const OUT_OF_WORLD_ERROR: CommandErrorType<0> = CommandErrorType::new(
    translation::java::ARGUMENT_POS_OUTOFWORLD,
    translation::java::ARGUMENT_POS_OUTOFWORLD,
);

#[derive(Clone, Copy, PartialEq, Eq)]
enum MaskMode {
    Replace,
    Masked,
    Filtered,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CloneMode {
    Normal,
    Force,
    Move,
}

struct CloneExecutor {
    mask_mode: MaskMode,
    clone_mode: CloneMode,
    has_filter: bool,
}

struct ClonedBlock {
    src_pos: BlockPos,
    dest_pos: BlockPos,
    state_id: BlockStateId,
    block_entity_nbt: Option<NbtCompound>,
}

fn inclusive_length(min: i32, max: i32) -> Option<u64> {
    let length = i64::from(max).checked_sub(i64::from(min))?.checked_add(1)?;
    u64::try_from(length).ok()
}

fn checked_volume(size_x: u64, size_y: u64, size_z: u64) -> Option<u64> {
    size_x.checked_mul(size_y)?.checked_mul(size_z)
}

fn offset_coordinate(origin: i32, offset: u64) -> Option<i32> {
    let offset = i64::try_from(offset).ok()?;
    let coordinate = i64::from(origin).checked_add(offset)?;
    i32::try_from(coordinate).ok()
}

fn clone_flags(fill_updates: bool) -> BlockFlags {
    if fill_updates {
        BlockFlags::NOTIFY_ALL
    } else {
        BlockFlags::FORCE_STATE | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK
    }
}

impl CommandExecutor for CloneExecutor {
    #[expect(clippy::too_many_lines)]
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let begin = BlockPosArgumentType::get_block_pos(context, ARG_BEGIN)?;
        let end = BlockPosArgumentType::get_block_pos(context, ARG_END)?;
        let dest = BlockPosArgumentType::get_block_pos(context, ARG_DEST)?;

        let filter_block = if self.has_filter {
            Some(BlockArgumentType::get(context, ARG_FILTER)?)
        } else {
            None
        };

        let min_x = begin.0.x.min(end.0.x);
        let max_x = begin.0.x.max(end.0.x);
        let min_y = begin.0.y.min(end.0.y);
        let max_y = begin.0.y.max(end.0.y);
        let min_z = begin.0.z.min(end.0.z);
        let max_z = begin.0.z.max(end.0.z);

        let size_x = inclusive_length(min_x, max_x).unwrap_or(u64::MAX);
        let size_y = inclusive_length(min_y, max_y).unwrap_or(u64::MAX);
        let size_z = inclusive_length(min_z, max_z).unwrap_or(u64::MAX);
        let volume = checked_volume(size_x, size_y, size_z).unwrap_or(u64::MAX);

        // carpet rule fillLimit caps the clone volume
        let fill_limit = crate::carpet::values()
            .fill_limit
            .clamp(1, i64::from(i32::MAX)) as u64;
        if volume > fill_limit {
            return Err(TOOBIG_ERROR.create_without_context_args_slice(&[
                TextComponent::text(fill_limit.to_string()),
                TextComponent::text(volume.to_string()),
            ]));
        }

        let dest_min_x = dest.0.x;
        let dest_min_y = dest.0.y;
        let dest_min_z = dest.0.z;
        let dest_max_x = offset_coordinate(dest_min_x, size_x.saturating_sub(1))
            .ok_or_else(|| OUT_OF_WORLD_ERROR.create_without_context())?;
        let dest_max_y = offset_coordinate(dest_min_y, size_y.saturating_sub(1))
            .ok_or_else(|| OUT_OF_WORLD_ERROR.create_without_context())?;
        let dest_max_z = offset_coordinate(dest_min_z, size_z.saturating_sub(1))
            .ok_or_else(|| OUT_OF_WORLD_ERROR.create_without_context())?;

        let overlap = !(dest_max_x < min_x
            || dest_min_x > max_x
            || dest_max_y < min_y
            || dest_min_y > max_y
            || dest_max_z < min_z
            || dest_min_z > max_z);

        if overlap && self.clone_mode != CloneMode::Force {
            return Err(OVERLAP_ERROR.create_without_context());
        }

        let world = context.source.world();

        let limit_check_pos_1 = BlockPos::new(min_x, min_y, min_z);
        let limit_check_pos_2 = BlockPos::new(max_x, max_y, max_z);
        let limit_check_pos_3 = BlockPos::new(dest_min_x, dest_min_y, dest_min_z);
        let limit_check_pos_4 = BlockPos::new(dest_max_x, dest_max_y, dest_max_z);

        if !world.is_in_build_limit(limit_check_pos_1)
            || !world.is_in_build_limit(limit_check_pos_2)
            || !world.is_in_build_limit(limit_check_pos_3)
            || !world.is_in_build_limit(limit_check_pos_4)
        {
            return Err(OUT_OF_WORLD_ERROR.create_without_context());
        }

        for cx in (min_x >> 4)..=(max_x >> 4) {
            for cz in (min_z >> 4)..=(max_z >> 4) {
                let chunk_pos = Vector2::new(cx, cz);
                if world.level.read_chunk_sync(&chunk_pos, |_| ()).is_none() {
                    return Err(NOT_LOADED_ERROR.create_without_context());
                }
            }
        }

        for cx in (dest_min_x >> 4)..=(dest_max_x >> 4) {
            for cz in (dest_min_z >> 4)..=(dest_max_z >> 4) {
                let chunk_pos = Vector2::new(cx, cz);
                if world.level.read_chunk_sync(&chunk_pos, |_| ()).is_none() {
                    return Err(NOT_LOADED_ERROR.create_without_context());
                }
            }
        }

        let mut blocks_to_clone = Vec::new();

        for x in 0..size_x {
            for y in 0..size_y {
                for z in 0..size_z {
                    let src_pos = BlockPos::new(
                        offset_coordinate(min_x, x)
                            .ok_or_else(|| OUT_OF_WORLD_ERROR.create_without_context())?,
                        offset_coordinate(min_y, y)
                            .ok_or_else(|| OUT_OF_WORLD_ERROR.create_without_context())?,
                        offset_coordinate(min_z, z)
                            .ok_or_else(|| OUT_OF_WORLD_ERROR.create_without_context())?,
                    );
                    let dest_pos = BlockPos::new(
                        offset_coordinate(dest_min_x, x)
                            .ok_or_else(|| OUT_OF_WORLD_ERROR.create_without_context())?,
                        offset_coordinate(dest_min_y, y)
                            .ok_or_else(|| OUT_OF_WORLD_ERROR.create_without_context())?,
                        offset_coordinate(dest_min_z, z)
                            .ok_or_else(|| OUT_OF_WORLD_ERROR.create_without_context())?,
                    );

                    let state_id = world.get_block_state_id(&src_pos);

                    let should_clone = match self.mask_mode {
                        MaskMode::Replace => true,
                        MaskMode::Masked => !verdantgolem_data::block_properties::is_air(state_id),
                        MaskMode::Filtered => {
                            let block = Block::from_state_id(state_id);
                            filter_block.is_some_and(|f| block.id == f.id)
                        }
                    };

                    if should_clone {
                        let block_entity_nbt = world.get_block_entity(&src_pos).map(|be| {
                            let mut nbt = NbtCompound::new();
                            be.write_internal(&mut nbt);
                            nbt
                        });

                        blocks_to_clone.push(ClonedBlock {
                            src_pos,
                            dest_pos,
                            state_id,
                            block_entity_nbt,
                        });
                    }
                }
            }
        }

        let flags = clone_flags(crate::carpet::values().fill_updates);
        let mut count = 0;
        for block in &blocks_to_clone {
            world.set_block_state(&block.dest_pos, block.state_id, flags);

            if let Some(nbt) = &block.block_entity_nbt {
                let mut new_nbt = nbt.clone();
                new_nbt.put_int("x", block.dest_pos.0.x);
                new_nbt.put_int("y", block.dest_pos.0.y);
                new_nbt.put_int("z", block.dest_pos.0.z);

                if let Some(be) = crate::block::entities::block_entity_from_nbt(&new_nbt) {
                    world.add_block_entity(be);
                }
            }

            count += 1;
        }

        if count == 0 {
            return Err(FAILED_ERROR.create_without_context());
        }

        let is_dest_pos = |pos: &BlockPos| {
            pos.0.x >= dest_min_x
                && pos.0.x <= dest_max_x
                && pos.0.y >= dest_min_y
                && pos.0.y <= dest_max_y
                && pos.0.z >= dest_min_z
                && pos.0.z <= dest_max_z
        };

        if self.clone_mode == CloneMode::Move {
            for block in &blocks_to_clone {
                if !is_dest_pos(&block.src_pos) {
                    world.set_block_state(&block.src_pos, BlockStateId::AIR, flags);
                }
            }
        }

        context.source.send_feedback(
            TextComponent::translate_cross(
                translation::java::COMMANDS_CLONE_SUCCESS,
                translation::bedrock::COMMANDS_CLONE_SUCCESS,
                [TextComponent::text(count.to_string())],
            ),
            true,
        );

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::{checked_volume, clone_flags, inclusive_length, offset_coordinate};
    use verdantgolem_world::world::BlockFlags;

    #[test]
    fn region_math_is_checked_and_wide() {
        assert_eq!(inclusive_length(-3, 3), Some(7));
        assert_eq!(
            inclusive_length(i32::MIN, i32::MAX),
            Some(u64::from(u32::MAX) + 1)
        );
        assert_eq!(checked_volume(u64::MAX, 2, 1), None);
        assert_eq!(offset_coordinate(i32::MAX, 1), None);
        assert_eq!(
            offset_coordinate(i32::MIN, u64::from(u32::MAX)),
            Some(i32::MAX)
        );
    }

    #[test]
    fn fill_updates_controls_callbacks_and_notifications() {
        assert_eq!(clone_flags(true), BlockFlags::NOTIFY_ALL);
        let quiet = clone_flags(false);
        assert!(quiet.contains(BlockFlags::FORCE_STATE));
        assert!(quiet.contains(BlockFlags::SKIP_BLOCK_ADDED_CALLBACK));
        assert!(!quiet.contains(BlockFlags::NOTIFY_NEIGHBORS));
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    dispatcher.register(
        command("clone", DESCRIPTION).requires(PERMISSION).then(
            argument(ARG_BEGIN, BlockPosArgumentType).then(
                argument(ARG_END, BlockPosArgumentType).then(
                    argument(ARG_DEST, BlockPosArgumentType)
                        .executes(CloneExecutor {
                            mask_mode: MaskMode::Replace,
                            clone_mode: CloneMode::Normal,
                            has_filter: false,
                        })
                        .then(
                            literal("replace")
                                .executes(CloneExecutor {
                                    mask_mode: MaskMode::Replace,
                                    clone_mode: CloneMode::Normal,
                                    has_filter: false,
                                })
                                .then(literal("force").executes(CloneExecutor {
                                    mask_mode: MaskMode::Replace,
                                    clone_mode: CloneMode::Force,
                                    has_filter: false,
                                }))
                                .then(literal("move").executes(CloneExecutor {
                                    mask_mode: MaskMode::Replace,
                                    clone_mode: CloneMode::Move,
                                    has_filter: false,
                                }))
                                .then(literal("normal").executes(CloneExecutor {
                                    mask_mode: MaskMode::Replace,
                                    clone_mode: CloneMode::Normal,
                                    has_filter: false,
                                })),
                        )
                        .then(
                            literal("masked")
                                .executes(CloneExecutor {
                                    mask_mode: MaskMode::Masked,
                                    clone_mode: CloneMode::Normal,
                                    has_filter: false,
                                })
                                .then(literal("force").executes(CloneExecutor {
                                    mask_mode: MaskMode::Masked,
                                    clone_mode: CloneMode::Force,
                                    has_filter: false,
                                }))
                                .then(literal("move").executes(CloneExecutor {
                                    mask_mode: MaskMode::Masked,
                                    clone_mode: CloneMode::Move,
                                    has_filter: false,
                                }))
                                .then(literal("normal").executes(CloneExecutor {
                                    mask_mode: MaskMode::Masked,
                                    clone_mode: CloneMode::Normal,
                                    has_filter: false,
                                })),
                        )
                        .then(
                            literal("filtered").then(
                                argument(ARG_FILTER, BlockArgumentType)
                                    .executes(CloneExecutor {
                                        mask_mode: MaskMode::Filtered,
                                        clone_mode: CloneMode::Normal,
                                        has_filter: true,
                                    })
                                    .then(literal("force").executes(CloneExecutor {
                                        mask_mode: MaskMode::Filtered,
                                        clone_mode: CloneMode::Force,
                                        has_filter: true,
                                    }))
                                    .then(literal("move").executes(CloneExecutor {
                                        mask_mode: MaskMode::Filtered,
                                        clone_mode: CloneMode::Move,
                                        has_filter: true,
                                    }))
                                    .then(literal("normal").executes(CloneExecutor {
                                        mask_mode: MaskMode::Filtered,
                                        clone_mode: CloneMode::Normal,
                                        has_filter: true,
                                    })),
                            ),
                        ),
                ),
            ),
        ),
    );
}
