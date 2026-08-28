use std::{collections::BTreeMap, fmt::Write as _};

use crate::TextComponent;

use crate::command::args::position_block::BlockPosArgumentConsumer;

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::argument;
use crate::command::{CommandExecutor, CommandResult, CommandSender};
use verdantgolem_data::entity::EntityType;
use verdantgolem_util::math::position::BlockPos;
use verdantgolem_util::math::vector2::Vector2;

const NAMES: [&str; 1] = ["perimeterinfo"];

const DESCRIPTION: &str = "Scans the surroundings for spots where monsters can spawn.";

const ARG_POS: &str = "pos";

/// Scan half-width; a 33x33 column scan keeps the command one-shot fast.
const SCAN_RADIUS: i32 = 16;

struct PerimeterInfoExecutor {
    needs_arg: bool,
}

impl CommandExecutor for PerimeterInfoExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs,
    ) -> CommandResult {
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
            player.living_entity.entity.block_pos.load()
        };
        let cx = center.0.x;
        let cz = center.0.z;
        let min_x = cx
            .checked_sub(SCAN_RADIUS)
            .ok_or_else(|| failed("scan area is outside the world".to_string()))?;
        let max_x = cx
            .checked_add(SCAN_RADIUS)
            .ok_or_else(|| failed("scan area is outside the world".to_string()))?;
        let min_z = cz
            .checked_sub(SCAN_RADIUS)
            .ok_or_else(|| failed("scan area is outside the world".to_string()))?;
        let max_z = cz
            .checked_add(SCAN_RADIUS)
            .ok_or_else(|| failed("scan area is outside the world".to_string()))?;

        let bottom_y = world.get_bottom_y();
        let top_y = world.get_top_y();
        for corner in [
            BlockPos::new(min_x, bottom_y, min_z),
            BlockPos::new(max_x, top_y, max_z),
        ] {
            if !world.is_in_build_limit(corner) {
                return Err(failed("scan area is outside the world".to_string()));
            }
        }
        for chunk_x in (min_x >> 4)..=(max_x >> 4) {
            for chunk_z in (min_z >> 4)..=(max_z >> 4) {
                if world
                    .level
                    .read_chunk_sync(&Vector2::new(chunk_x, chunk_z), |_| ())
                    .is_none()
                {
                    return Err(failed(format!(
                        "scan area contains unloaded chunk [{chunk_x}, {chunk_z}]"
                    )));
                }
            }
        }

        // OnGround monster representative (zombie) for spawnability.
        let entity_type = &EntityType::ZOMBIE;
        let is_thundering = world.is_thundering();
        let mut spawnable = 0u64;
        let mut per_y: BTreeMap<i32, u64> = BTreeMap::new();
        for dx in -SCAN_RADIUS..=SCAN_RADIUS {
            for dz in -SCAN_RADIUS..=SCAN_RADIUS {
                for dy in bottom_y..=top_y {
                    let pos = BlockPos::new(cx + dx, dy, cz + dz);
                    if crate::world::natural_spawner::is_valid_spawn_position_for_type(
                        &world,
                        &pos,
                        entity_type.category,
                        entity_type,
                        0.0,
                        is_thundering,
                    ) {
                        spawnable = spawnable.saturating_add(1);
                        let count = per_y.entry(dy).or_default();
                        *count = count.saturating_add(1);
                    }
                }
            }
        }
        let mut per_y: Vec<_> = per_y.into_iter().collect();
        per_y.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        let mut message = format!(
            "Spawnable spots for monsters within {SCAN_RADIUS} blocks of {center}: {spawnable}"
        );
        if spawnable > 0 {
            message.push_str("\nHighest chance levels:");
            for (y, count) in per_y.iter().take(5) {
                let _ = write!(message, "\ny={y}: {count} spots");
            }
        } else {
            message.push_str("\nThe perimeter is fully spawnproof!");
        }

        sender.send_message(TextComponent::text(message));

        Ok(i32::try_from(spawnable).unwrap_or(i32::MAX))
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
