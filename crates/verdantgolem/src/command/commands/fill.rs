use std::sync::Arc;

use crate::command::args::block::{
    BlockArgumentConsumer, BlockPredicate, BlockPredicateArgumentConsumer,
};
use crate::command::args::position_block::BlockPosArgumentConsumer;
use crate::command::args::{ConsumedArgs, FindArg};
use crate::command::tree::CommandTree;
use crate::command::tree::builder::{argument, literal};
use crate::command::{CommandError, CommandExecutor, CommandResult, CommandSender};
use crate::world::World;

use verdantgolem_data::translation;
use verdantgolem_data::{Block, BlockStateId};
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_util::math::vector3::Vector3;
use verdantgolem_util::text::TextComponent;
use verdantgolem_world::world::BlockFlags;

const NAMES: [&str; 1] = ["fill"];

const DESCRIPTION: &str = "Fills all or parts of a region with a specific block.";

const ARG_BLOCK: &str = "block";
const ARG_FROM: &str = "from";
const ARG_TO: &str = "to";
const ARG_FILTER: &str = "filter";

#[derive(Clone, Copy, Default)]
enum Mode {
    /// Destroys blocks with particles and item drops
    Destroy,
    /// Leaves only the outer layer of blocks, removes the inner ones (creates a hollow space)
    Hollow,
    /// Only replaces air blocks, keeping non-air blocks unchanged
    Keep,
    /// Like Hollow but doesn't replace inner blocks with air, just the outline
    Outline,
    /// Replaces all blocks with the new block state, without particles
    #[default]
    Replace,
    /// Replaces all blocks with the new block state, without particles and neighbors update
    Strict,
}

struct Executor(Mode);

fn fill_flags(fill_updates: bool) -> BlockFlags {
    if fill_updates {
        BlockFlags::FORCE_STATE
    } else {
        BlockFlags::FORCE_STATE | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK
    }
}

fn inclusive_span(min: i32, max: i32) -> Option<u64> {
    let span = i64::from(max).checked_sub(i64::from(min))?.checked_add(1)?;
    u64::try_from(span).ok()
}

fn checked_region_volume(min: Vector3<i32>, max: Vector3<i32>) -> Option<u64> {
    inclusive_span(min.x, max.x)?
        .checked_mul(inclusive_span(min.y, max.y)?)?
        .checked_mul(inclusive_span(min.z, max.z)?)
}

fn not_in_filter(filter: &BlockPredicate, old_block: &Block) -> bool {
    match filter {
        BlockPredicate::Tag(tag) => !tag.contains(&old_block.id.as_u16()),
        BlockPredicate::Block(block) => *block != old_block.id,
    }
}

enum FillerResult {
    DidNotPlaceBlock = 0,
    PlacedBlock = 1,
    PlacedBlockWithoutUpdate = 2,
}

struct Context {
    block_state_id: BlockStateId,
    option_filter: Option<BlockPredicate>,
    world: Arc<World>,
    block_flags: BlockFlags,
    placed_blocks: i32,
    to_update: Vec<BlockPos>,

    start_x: i32,
    start_y: i32,
    start_z: i32,
    end_x: i32,
    end_y: i32,
    end_z: i32,
}

impl Context {
    /// Checks whether the block position is at the edge of the region stored by this context.
    #[inline]
    const fn is_edge(&self, block_position: BlockPos) -> bool {
        let pos = block_position.0;
        pos.x == self.start_x
            || pos.x == self.end_x
            || pos.y == self.start_y
            || pos.y == self.end_y
            || pos.z == self.start_z
            || pos.z == self.end_z
    }
}

trait Filler {
    fn execute_for_pos(context: &Context, block_position: BlockPos) -> FillerResult;

    fn execute_for_region(context: &mut Context) {
        for x in context.start_x..=context.end_x {
            for y in context.start_y..=context.end_y {
                for z in context.start_z..=context.end_z {
                    let block_position = BlockPos(Vector3::new(x, y, z));
                    let filler_result = Self::execute_for_pos(context, block_position);
                    match filler_result {
                        FillerResult::PlacedBlock => {
                            context.placed_blocks += 1;
                            context.to_update.push(block_position);
                        }
                        FillerResult::PlacedBlockWithoutUpdate => {
                            context.placed_blocks += 1;
                        }
                        FillerResult::DidNotPlaceBlock => {}
                    }
                }
            }
        }
    }
}

struct DestroyFiller;
impl Filler for DestroyFiller {
    fn execute_for_pos(context: &Context, block_position: BlockPos) -> FillerResult {
        if let Some(filter) = &context.option_filter
            && not_in_filter(filter, context.world.get_block(&block_position))
        {
            return FillerResult::DidNotPlaceBlock;
        }
        context.world.break_block(
            &block_position,
            None,
            BlockFlags::SKIP_DROPS | context.block_flags,
        );
        context
            .world
            .set_block_state(&block_position, context.block_state_id, context.block_flags);
        FillerResult::PlacedBlock
    }
}

struct HollowFiller;
impl Filler for HollowFiller {
    fn execute_for_pos(context: &Context, block_position: BlockPos) -> FillerResult {
        if let Some(filter) = &context.option_filter
            && not_in_filter(filter, context.world.get_block(&block_position))
        {
            return FillerResult::DidNotPlaceBlock;
        }
        if context.is_edge(block_position) {
            context.world.set_block_state(
                &block_position,
                context.block_state_id,
                context.block_flags,
            );
        } else {
            context
                .world
                .set_block_state(&block_position, BlockStateId::AIR, context.block_flags);
        }
        FillerResult::PlacedBlock
    }
}

struct KeepFiller;
impl Filler for KeepFiller {
    fn execute_for_pos(context: &Context, block_position: BlockPos) -> FillerResult {
        let (old_block, old_state) = context.world.get_block_and_state(&block_position);
        if old_state.is_air() {
            if let Some(filter) = &context.option_filter
                && not_in_filter(filter, old_block)
            {
                return FillerResult::DidNotPlaceBlock;
            }
            context.world.set_block_state(
                &block_position,
                context.block_state_id,
                context.block_flags,
            );
            FillerResult::PlacedBlock
        } else {
            FillerResult::DidNotPlaceBlock
        }
    }
}

struct OutlineFiller;
impl Filler for OutlineFiller {
    fn execute_for_pos(context: &Context, block_position: BlockPos) -> FillerResult {
        if !context.is_edge(block_position) {
            return FillerResult::DidNotPlaceBlock;
        }
        if let Some(filter) = &context.option_filter
            && not_in_filter(filter, context.world.get_block(&block_position))
        {
            return FillerResult::DidNotPlaceBlock;
        }
        context
            .world
            .set_block_state(&block_position, context.block_state_id, context.block_flags);
        FillerResult::PlacedBlock
    }
}

struct ReplaceFiller;
impl Filler for ReplaceFiller {
    fn execute_for_pos(context: &Context, block_position: BlockPos) -> FillerResult {
        if let Some(filter) = &context.option_filter
            && not_in_filter(filter, context.world.get_block(&block_position))
        {
            return FillerResult::DidNotPlaceBlock;
        }
        context
            .world
            .set_block_state(&block_position, context.block_state_id, context.block_flags);
        FillerResult::PlacedBlock
    }
}

struct StrictFiller;
impl Filler for StrictFiller {
    fn execute_for_pos(context: &Context, block_position: BlockPos) -> FillerResult {
        if let Some(filter) = &context.option_filter
            && not_in_filter(filter, context.world.get_block(&block_position))
        {
            return FillerResult::DidNotPlaceBlock;
        }
        context.world.set_block_state(
            &block_position,
            context.block_state_id,
            BlockFlags::SKIP_BLOCK_ADDED_CALLBACK,
        );
        FillerResult::PlacedBlockWithoutUpdate
    }
}

impl CommandExecutor for Executor {
    fn execute(
        &self,
        sender: &CommandSender,
        server: &crate::server::Server,
        args: &ConsumedArgs,
    ) -> CommandResult {
        let block = BlockArgumentConsumer::find_arg(args, ARG_BLOCK)?;
        let block_state_id = block.default_state.id;
        let from = BlockPosArgumentConsumer::find_arg(args, ARG_FROM)?;
        let to = BlockPosArgumentConsumer::find_arg(args, ARG_TO)?;
        let mode = self.0;
        let fill_updates = crate::carpet::values().fill_updates;

        let mut context = Context {
            block_state_id,
            option_filter: BlockPredicateArgumentConsumer::find_arg(args, ARG_FILTER)?,
            world: sender
                .world_or_first(server)
                .ok_or(CommandError::InvalidRequirement)?,
            block_flags: fill_flags(fill_updates),
            placed_blocks: 0,
            to_update: Vec::new(),

            start_x: from.0.x.min(to.0.x),
            start_y: from.0.y.min(to.0.y),
            start_z: from.0.z.min(to.0.z),

            end_x: from.0.x.max(to.0.x),
            end_y: from.0.y.max(to.0.y),
            end_z: from.0.z.max(to.0.z),
        };

        if !context.world.is_in_build_limit(from) || !context.world.is_in_build_limit(to) {
            return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                translation::java::ARGUMENT_POS_OUTOFBOUNDS,
                translation::java::ARGUMENT_POS_OUTOFBOUNDS,
                [],
            )));
        }

        let max_block_modifications = {
            let level_info = server.level_info.load();
            level_info.game_rules.max_block_modifications
        };

        let total_blocks = checked_region_volume(
            Vector3::new(context.start_x, context.start_y, context.start_z),
            Vector3::new(context.end_x, context.end_y, context.end_z),
        )
        .unwrap_or(u64::MAX);

        if total_blocks > u64::try_from(max_block_modifications).unwrap_or(0) {
            return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                translation::java::COMMANDS_FILL_TOOBIG,
                translation::java::COMMANDS_FILL_TOOBIG,
                [
                    TextComponent::text(max_block_modifications.to_string()),
                    TextComponent::text(total_blocks.to_string()),
                ],
            )));
        }

        // carpet rule fillLimit caps the fill volume in addition to the gamerule.
        let fill_limit = crate::carpet::values().fill_limit;
        if total_blocks > u64::try_from(fill_limit).unwrap_or(0) {
            return Err(CommandError::CommandFailed(TextComponent::text(format!(
                "Too many blocks to fill: {total_blocks} is larger than fillLimit {fill_limit}"
            ))));
        }

        match mode {
            Mode::Destroy => DestroyFiller::execute_for_region(&mut context),
            Mode::Replace => ReplaceFiller::execute_for_region(&mut context),
            Mode::Keep => KeepFiller::execute_for_region(&mut context),
            Mode::Hollow => HollowFiller::execute_for_region(&mut context),
            Mode::Outline => OutlineFiller::execute_for_region(&mut context),
            Mode::Strict => StrictFiller::execute_for_region(&mut context),
        }

        // carpet rule fillUpdates: false suppresses neighbor updates from /fill.
        if fill_updates {
            for i in context.to_update {
                context.world.update_neighbors(&i, None);
            }
        }

        if context.placed_blocks == 0 {
            return Err(CommandError::CommandFailed(TextComponent::translate_cross(
                translation::java::COMMANDS_FILL_FAILED,
                translation::bedrock::COMMANDS_FILL_FAILED,
                [],
            )));
        }

        sender.send_message(TextComponent::translate_cross(
            translation::java::COMMANDS_FILL_SUCCESS,
            translation::bedrock::COMMANDS_FILL_SUCCESS,
            [TextComponent::text(context.placed_blocks.to_string())],
        ));

        Ok(context.placed_blocks)
    }
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION).then(
        argument(ARG_FROM, BlockPosArgumentConsumer).then(
            argument(ARG_TO, BlockPosArgumentConsumer).then(
                argument(ARG_BLOCK, BlockArgumentConsumer)
                    .then(literal("destroy").execute(Executor(Mode::Destroy)))
                    .then(literal("hollow").execute(Executor(Mode::Hollow)))
                    .then(literal("keep").execute(Executor(Mode::Keep)))
                    .then(literal("outline").execute(Executor(Mode::Outline)))
                    .then(
                        literal("replace")
                            .then(
                                argument(ARG_FILTER, BlockPredicateArgumentConsumer)
                                    .then(literal("destroy").execute(Executor(Mode::Destroy)))
                                    .then(literal("hollow").execute(Executor(Mode::Hollow)))
                                    .then(literal("keep").execute(Executor(Mode::Keep)))
                                    .then(literal("outline").execute(Executor(Mode::Outline)))
                                    .then(literal("strict").execute(Executor(Mode::Strict)))
                                    .execute(Executor(Mode::Replace)),
                            )
                            .execute(Executor(Mode::Replace)),
                    )
                    .then(literal("strict").execute(Executor(Mode::Strict)))
                    .execute(Executor(Mode::Replace)),
            ),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::{checked_region_volume, fill_flags};
    use verdantgolem_util::math::vector3::Vector3;
    use verdantgolem_world::world::BlockFlags;

    #[test]
    fn volume_math_is_checked_and_wide() {
        assert_eq!(
            checked_region_volume(Vector3::new(-1, 0, -2), Vector3::new(1, 3, 2)),
            Some(60)
        );
        assert_eq!(
            checked_region_volume(
                Vector3::new(i32::MIN, i32::MIN, i32::MIN),
                Vector3::new(i32::MAX, i32::MAX, i32::MAX)
            ),
            None
        );
    }

    #[test]
    fn disabled_updates_skip_added_callbacks() {
        assert_eq!(fill_flags(true), BlockFlags::FORCE_STATE);
        let quiet = fill_flags(false);
        assert!(quiet.contains(BlockFlags::FORCE_STATE));
        assert!(quiet.contains(BlockFlags::SKIP_BLOCK_ADDED_CALLBACK));
        assert!(!quiet.contains(BlockFlags::NOTIFY_NEIGHBORS));
    }
}
