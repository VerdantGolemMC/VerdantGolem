use crate::TextComponent;

use crate::command::args::position_block::BlockPosArgumentConsumer;

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::argument;
use crate::command::{CommandExecutor, CommandResult, CommandSender};
use verdantgolem_data::entity::EntityType;
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_util::math::vector3::Vector3;

const NAMES: [&str; 1] = ["perimeterinfo"];

const DESCRIPTION: &str = "Scans the surroundings for spots where monsters can spawn.";

const ARG_POS: &str = "pos";

/// Scan half-width; a 33x33 column scan keeps the command one-shot fast.
const SCAN_RADIUS: i32 = 16;

struct PerimeterInfoExecutor {
    needs_arg: bool,
}

impl CommandExecutor for PerimeterInfoExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a crate::server::Server,
        args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let Some(world) = sender.world() else {
                return Err(failed(
                    "perimeterinfo must be run with a world context".to_string(),
                ));
            };
            let center: BlockPos = if self.needs_arg {
                BlockPosArgumentConsumer::find_loaded_arg(args, ARG_POS, &world)?
            } else {
                let Some(player) = sender.as_player() else {
                    return Err(failed(
                        "console must pass a position: /perimeterinfo <pos>".to_string(),
                    ));
                };
                player.get_entity().block_pos.load()
            };
            let cx = center.0.x;
            let cz = center.0.z;

            // OnGround monster representative (zombie) for spawnability.
            let entity_type = &EntityType::ZOMBIE;
            let min_y = world.level_info.load().min_y;
            let mut spawnable = 0u32;
            let mut per_y: Vec<(i32, u32)> = Vec::new();
            for dx in -SCAN_RADIUS..=SCAN_RADIUS {
                for dz in -SCAN_RADIUS..=SCAN_RADIUS {
                    for dy in min_y..min_y + 128 {
                        let pos = BlockPos(Vector3::new(cx + dx, dy, cz + dz));
                        if crate::world::natural_spawner::is_spawn_position_ok(
                            &world,
                            &pos,
                            entity_type,
                        ) {
                            spawnable += 1;
                            if let Some(entry) = per_y.iter_mut().find(|(y, _)| *y == dy) {
                                entry.1 += 1;
                            } else {
                                per_y.push((dy, 1));
                            }
                        }
                    }
                }
            }
            per_y.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

            let mut message = format!(
                "Spawnable spots for monsters within {SCAN_RADIUS} blocks of {center}: {spawnable}"
            );
            if spawnable > 0 {
                message.push_str("\nHighest chance levels:");
                for (y, count) in per_y.iter().take(5) {
                    message.push_str(&format!("\ny={y}: {count} spots"));
                }
            } else {
                message.push_str("\nThe perimeter is fully spawnproof!");
            }

            sender.send_message(TextComponent::text(message)).await;

            Ok(spawnable as i32)
        })
    }
}

fn failed(message: String) -> crate::command::dispatcher::CommandError {
    crate::command::dispatcher::CommandError::CommandFailed(TextComponent::text(message))
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .execute(PerimeterInfoExecutor { needs_arg: false })
        .then(
            argument(ARG_POS, BlockPosArgumentConsumer)
                .execute(PerimeterInfoExecutor { needs_arg: true }),
        )
}
