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

impl CommandExecutor for DrawExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a crate::server::Server,
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

            // carpet rule fillUpdates also applies to /draw
            let flags = if crate::carpet::values().fill_updates {
                BlockFlags::FORCE_STATE | BlockFlags::NOTIFY_NEIGHBORS
            } else {
                BlockFlags::FORCE_STATE
            };

            let radius_sq = f64::from(radius).powi(2);
            let shell_inner = (f64::from(radius) - 1.5).powi(2);
            let mut placed = 0u32;
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
                        let pos = BlockPos(center.0.add_raw(dx, dy, dz));
                        let old = world.get_block_state(&pos);
                        if old.is_air() {
                            world
                                .set_block_state(&pos, block.default_state.id, flags)
                                .await;
                            placed += 1;
                        }
                    }
                }
            }

            let shape = if self.filled { "ball" } else { "sphere" };
            sender
                .send_message(TextComponent::text(format!(
                    "Drew a {shape} of radius {radius}: {placed} blocks placed"
                )))
                .await;

            Ok(placed as i32)
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
