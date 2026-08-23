use crate::TextComponent;

use crate::command::args::FindArg;
use crate::command::args::block::BlockArgumentConsumer;
use crate::command::args::position_block::BlockPosArgumentConsumer;
use crate::command::args::simple::SimpleArgConsumer;

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::{argument, literal};
use crate::command::{CommandExecutor, CommandResult, CommandSender};
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_util::math::vector2::Vector2;
use verdantgolem_world::world::BlockFlags;

const NAMES: [&str; 1] = ["draw"];

const DESCRIPTION: &str = "Draws geometric shapes of blocks.";

const ARG_CENTER: &str = "center";
const ARG_RADIUS: &str = "radius";
const ARG_BLOCK: &str = "block";

/// Maximum radius to keep the command snappy.
const MAX_RADIUS: i32 = 16;

struct DrawExecutor {
    filled: bool,
}

fn draw_flags(fill_updates: bool) -> BlockFlags {
    if fill_updates {
        BlockFlags::NOTIFY_ALL
    } else {
        BlockFlags::FORCE_STATE | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK
    }
}

impl CommandExecutor for DrawExecutor {
    #[expect(clippy::too_many_lines)]
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let Some(world) = sender.world() else {
                return Err(failed("draw must be run with a world context".to_string()));
            };
            let center: BlockPos =
                BlockPosArgumentConsumer::find_loaded_arg(args, ARG_CENTER, &world)?;
            let radius: i32 = SimpleArgConsumer::find_arg(args, ARG_RADIUS)?
                .parse()
                .map_err(|_| invalid(ARG_RADIUS))?;
            if !(1..=MAX_RADIUS).contains(&radius) {
                return Err(failed(format!("radius must be between 1 and {MAX_RADIUS}")));
            }
            let block = BlockArgumentConsumer::find_arg(args, ARG_BLOCK)?;

            let radius_sq = f64::from(radius).powi(2);
            let shell_inner = (f64::from(radius) - 1.5).powi(2);
            let mut targets = Vec::new();
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    for dz in -radius..=radius {
                        let dist_sq = f64::from(dx * dx + dy * dy + dz * dz);
                        let in_shape = if self.filled {
                            dist_sq <= radius_sq
                        } else {
                            dist_sq <= radius_sq && dist_sq >= shell_inner
                        };
                        if !in_shape {
                            continue;
                        }
                        let x = center
                            .0
                            .x
                            .checked_add(dx)
                            .ok_or_else(|| failed("shape is outside the world".to_string()))?;
                        let y = center
                            .0
                            .y
                            .checked_add(dy)
                            .ok_or_else(|| failed("shape is outside the world".to_string()))?;
                        let z = center
                            .0
                            .z
                            .checked_add(dz)
                            .ok_or_else(|| failed("shape is outside the world".to_string()))?;
                        targets.push(BlockPos::new(x, y, z));
                    }
                }
            }

            if targets.iter().any(|pos| !world.is_in_build_limit(*pos)) {
                return Err(failed("shape is outside the world".to_string()));
            }
            let min_x = center
                .0
                .x
                .checked_sub(radius)
                .ok_or_else(|| failed("shape is outside the world".to_string()))?;
            let max_x = center
                .0
                .x
                .checked_add(radius)
                .ok_or_else(|| failed("shape is outside the world".to_string()))?;
            let min_z = center
                .0
                .z
                .checked_sub(radius)
                .ok_or_else(|| failed("shape is outside the world".to_string()))?;
            let max_z = center
                .0
                .z
                .checked_add(radius)
                .ok_or_else(|| failed("shape is outside the world".to_string()))?;
            for chunk_x in (min_x >> 4)..=(max_x >> 4) {
                for chunk_z in (min_z >> 4)..=(max_z >> 4) {
                    if world
                        .level
                        .read_chunk_sync(&Vector2::new(chunk_x, chunk_z), |_| ())
                        .is_none()
                    {
                        return Err(failed(format!(
                            "shape contains unloaded chunk [{chunk_x}, {chunk_z}]"
                        )));
                    }
                }
            }

            let target_count = i64::try_from(targets.len()).unwrap_or(i64::MAX);
            let fill_limit = crate::carpet::values().fill_limit.max(1);
            if target_count > fill_limit {
                return Err(failed(format!(
                    "shape contains {target_count} blocks, exceeding fillLimit {fill_limit}"
                )));
            }
            let vanilla_limit = server.level_info.load().game_rules.max_block_modifications;
            if target_count > vanilla_limit {
                return Err(failed(format!(
                    "shape contains {target_count} blocks, exceeding max block modifications {vanilla_limit}"
                )));
            }

            // Preserve the command's air-only placement semantics, but determine
            // every real state change before the first write.
            let target_state = block.default_state.id;
            targets.retain(|pos| {
                let old = world.get_block_state(pos);
                old.is_air() && old.id != target_state
            });

            let flags = draw_flags(crate::carpet::values().fill_updates);
            let mut placed = 0usize;
            for pos in targets {
                let replaced = world.set_block_state(&pos, target_state, flags).await;
                if replaced != target_state {
                    placed += 1;
                }
            }

            let shape = if self.filled { "ball" } else { "sphere" };
            sender
                .send_message(TextComponent::text(format!(
                    "Drew a {shape} of radius {radius}: {placed} blocks placed"
                )))
                .await;

            Ok(i32::try_from(placed).unwrap_or(i32::MAX))
        })
    }
}

fn invalid(arg: &str) -> crate::command::dispatcher::CommandError {
    failed(format!("Invalid value for {arg}"))
}

fn failed(message: String) -> crate::command::dispatcher::CommandError {
    crate::command::dispatcher::CommandError::CommandFailed(TextComponent::text(message))
}

fn shape_args(executor: DrawExecutor) -> crate::command::tree::builder::NonLeafNodeBuilder {
    argument(ARG_CENTER, BlockPosArgumentConsumer).then(
        argument(ARG_RADIUS, SimpleArgConsumer)
            .then(argument(ARG_BLOCK, BlockArgumentConsumer).execute(executor)),
    )
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .then(literal("sphere").then(shape_args(DrawExecutor { filled: false })))
        .then(literal("ball").then(shape_args(DrawExecutor { filled: true })))
}

#[cfg(test)]
mod tests {
    use super::draw_flags;
    use verdantgolem_world::world::BlockFlags;

    #[test]
    fn fill_updates_controls_callbacks_and_notifications() {
        assert_eq!(draw_flags(true), BlockFlags::NOTIFY_ALL);
        let quiet = draw_flags(false);
        assert!(quiet.contains(BlockFlags::FORCE_STATE));
        assert!(quiet.contains(BlockFlags::SKIP_BLOCK_ADDED_CALLBACK));
        assert!(!quiet.contains(BlockFlags::NOTIFY_NEIGHBORS));
    }
}
