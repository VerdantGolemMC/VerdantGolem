use std::fmt::Write as _;

use crate::TextComponent;

use crate::command::args::position_block::BlockPosArgumentConsumer;
use verdantgolem_data::tag::Taggable;

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::argument;
use crate::command::{CommandExecutor, CommandResult, CommandSender};
use verdantgolem_nbt::compound::NbtCompound;
use verdantgolem_util::math::position::BlockPos;

const NAMES: [&str; 1] = ["info"];

const DESCRIPTION: &str = "Shows the block, state and block-entity data at a position.";

const ARG_POS: &str = "pos";

struct InfoExecutor;

impl CommandExecutor for InfoExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let Some(world) = sender.world() else {
                return Err(failed("info must be run with a world context".to_string()));
            };
            let pos: BlockPos = BlockPosArgumentConsumer::find_loaded_arg(args, ARG_POS, &world)?;

            let (block, state) = world.get_block_and_state(&pos);
            let mut message = format!(
                "Block at {}: {} (state {})",
                pos,
                block.registry_key(),
                state.id.as_u16()
            );

            if let Some(block_entity) = world.get_block_entity(&pos) {
                let mut nbt = NbtCompound::new();
                block_entity.write_nbt(&mut nbt).await;
                let _ = writeln!(message, "Block entity: {nbt:?}");
            }

            sender.send_message(TextComponent::text(message)).await;

            Ok(1)
        })
    }
}

fn failed(message: String) -> crate::command::dispatcher::CommandError {
    crate::command::dispatcher::CommandError::CommandFailed(TextComponent::text(message))
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .then(argument(ARG_POS, BlockPosArgumentConsumer).execute(InfoExecutor))
}
