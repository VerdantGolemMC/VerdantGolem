use crate::TextComponent;

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::literal;
use crate::command::{CommandExecutor, CommandResult, CommandSender};
use verdantgolem_data::dimension::Dimension;
use verdantgolem_data::entity::MobCategory;

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

struct MobCapsExecutor;

impl CommandExecutor for MobCapsExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        server: &crate::server::Server,
        _args: &ConsumedArgs,
    ) -> CommandResult {
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
                let cap = crate::world::natural_spawner::scaled_mob_cap(
                    category.max,
                    spawn_state.spawnable_chunk_count(),
                    multiplier,
                );
                format!("\n{}: {}/{}", CATEGORY_NAMES[category.id], count, cap)
            };
            message.push_str(&line);
        }

        sender.send_message(TextComponent::text(message));

        Ok(1)
    }
}

struct TrackingExecutor;

impl CommandExecutor for TrackingExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        _args: &ConsumedArgs,
    ) -> CommandResult {
        let message = match crate::carpet::spawn_tracking::toggle() {
            crate::carpet::spawn_tracking::ToggleResult::Started => {
                "Started spawn tracking. Run /spawn tracking again to stop and report.".to_string()
            }
            crate::carpet::spawn_tracking::ToggleResult::Stopped(report) => report,
        };
        sender.send_message(TextComponent::text(message));
        Ok(1)
    }
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .then(literal("mobcaps").execute(MobCapsExecutor))
        .then(literal("tracking").execute(TrackingExecutor))
}
