use crate::TextComponent;

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::literal;
use crate::command::{CommandExecutor, CommandResult, CommandSender};
use verdantgolem_data::entity_type::MobCategory;
use verdantgolem_world::dimension::Dimension;

const NAMES: [&str; 1] = ["spawn"];

const DESCRIPTION: &str = "Spawn statistics for technical farms.";

/// Display names for `MobCategory::SPAWNING_CATEGORIES`, indexed by `MobCategory::id`.
const CATEGORY_NAMES: [&str; 8] = [
    "monster",
    "creature",
    "ambient",
    "axolotls",
    "underground_water_creature",
    "water_creature",
    "water_ambient",
    "misc",
];

const MAGIC_NUMBER: i32 = 289;

struct MobCapsExecutor;

impl CommandExecutor for MobCapsExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        server: &'a crate::server::Server,
        _args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let world = sender
                .world()
                .unwrap_or_else(|| server.get_world_from_dimension(&Dimension::OVERWORLD));
            let spawn_state = world.spawn_state.load();
            let multiplier = crate::carpet::values().mob_cap_multiplier;

            let mut message = format!(
                "Mobcaps ({} spawnable chunks):",
                spawn_state.spawnable_chunk_count()
            );
            for category in MobCategory::SPAWNING_CATEGORIES {
                let count = spawn_state.category_count(category);
                let line = if category.max < 0 {
                    format!("\n{}: {}", CATEGORY_NAMES[category.id], count)
                } else {
                    let cap = (f64::from(category.max) * multiplier) as i32
                        * spawn_state.spawnable_chunk_count()
                        / MAGIC_NUMBER;
                    format!("\n{}: {}/{}", CATEGORY_NAMES[category.id], count, cap)
                };
                message.push_str(&line);
            }

            sender.send_message(TextComponent::text(message)).await;

            Ok(1)
        })
    }
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION).then(literal("mobcaps").execute(MobCapsExecutor))
}
