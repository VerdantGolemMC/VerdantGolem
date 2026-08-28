use crate::TextComponent;
use crate::carpet::fake_player;

use crate::command::args::FindArg;
use crate::command::args::simple::SimpleArgConsumer;

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::{argument, literal};
use crate::command::{CommandExecutor, CommandResult, CommandSender};

const NAMES: [&str; 1] = ["player"];

const DESCRIPTION: &str = "Manages carpet-style fake players.";

const ARG_NAME: &str = "name";
const ARG_YAW: &str = "yaw";
const ARG_PITCH: &str = "pitch";

struct ListExecutor;

impl CommandExecutor for ListExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        _args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let names = fake_player::list();
        let message = if names.is_empty() {
            "No fake players are online.".to_string()
        } else {
            format!("Fake players ({}): {}", names.len(), names.join(", "))
        };
        sender.send_message(TextComponent::text(message));
        Ok(names.len() as i32)
    }
}

struct SpawnExecutor;

impl CommandExecutor for SpawnExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_NAME)?.to_string();
        let Some(world) = sender.world() else {
            return Err(crate::command::dispatcher::CommandError::CommandFailed(
                TextComponent::text("Fake players must be spawned by an in-game player."),
            ));
        };
        let executor = sender.as_player();

        let (position, yaw, pitch) = executor.as_ref().map_or_else(
            || {
                let info = world.level_info.load();
                (
                    verdantgolem_util::math::vector3::Vector3::new(
                        f64::from(info.spawn_x) + 0.5,
                        f64::from(info.spawn_y),
                        f64::from(info.spawn_z) + 0.5,
                    ),
                    info.spawn_yaw,
                    0.0,
                )
            },
            |player| {
                let entity = &player.living_entity.entity;
                (entity.pos.load(), entity.yaw.load(), entity.pitch.load())
            },
        );

        let sender = sender.clone();
        let server = world.server.upgrade().ok_or_else(|| {
            crate::command::dispatcher::CommandError::CommandFailed(TextComponent::text(
                "Server is inactive",
            ))
        })?;
        server.spawn_task(async move {
            match fake_player::spawn(&world, &name, position, yaw, pitch).await {
                Ok(()) => {
                    sender.send_message(TextComponent::text(format!("Spawned fake player {name}")))
                }
                Err(error) => sender.send_message(
                    TextComponent::text(error)
                        .color_named(verdantgolem_util::text::color::NamedColor::Red),
                ),
            }
        });
        Ok(1)
    }
}

struct KillExecutor;

impl CommandExecutor for KillExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        server: &crate::server::Server,
        args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_NAME)?.to_string();
        // Fail synchronously for unknown names; the teardown itself performs async
        // chunk cleanup and advancement persistence on the server task tracker.
        if fake_player::get(&name).is_none() {
            return Err(unknown(&name));
        }
        let sender = sender.clone();
        let Some(server) = sender
            .world_or_first(server)
            .and_then(|world| world.server.upgrade())
        else {
            return Err(crate::command::dispatcher::CommandError::CommandFailed(
                TextComponent::text("Server is inactive"),
            ));
        };
        let task_server = server.clone();
        server.spawn_task(async move {
            match fake_player::kill(&task_server, &name).await {
                Ok(()) => {
                    sender.send_message(TextComponent::text(format!("Removed fake player {name}")));
                }
                Err(error) => sender.send_message(
                    TextComponent::text(error)
                        .color_named(verdantgolem_util::text::color::NamedColor::Red),
                ),
            }
        });
        Ok(1)
    }
}

struct LookExecutor;

impl CommandExecutor for LookExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_NAME)?;
        let yaw: f32 = SimpleArgConsumer::find_arg(args, ARG_YAW)?
            .parse()
            .map_err(|_| invalid_value(ARG_YAW))?;
        let pitch: f32 = SimpleArgConsumer::find_arg(args, ARG_PITCH)?
            .parse()
            .map_err(|_| invalid_value(ARG_PITCH))?;

        let Some(player) = fake_player::get(name) else {
            return Err(unknown(name));
        };
        fake_player::look_up(&player, yaw, pitch);
        sender.send_message(TextComponent::text(format!(
            "Turned fake player {name} to yaw {yaw}, pitch {pitch}"
        )));
        Ok(1)
    }
}

struct AttackExecutor;

impl CommandExecutor for AttackExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_NAME)?;
        let now_attacking = !fake_player::is_attacking(name);
        fake_player::set_attacking(name, now_attacking).map_err(text_error)?;
        sender.send_message(TextComponent::text(format!(
            "{} attacking for {name}",
            if now_attacking { "Started" } else { "Stopped" }
        )));
        Ok(1)
    }
}

fn text_error(error: String) -> crate::command::dispatcher::CommandError {
    crate::command::dispatcher::CommandError::CommandFailed(TextComponent::text(error))
}

struct SneakExecutor;

impl CommandExecutor for SneakExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_NAME)?;
        let player = fake_player::get(name).ok_or_else(|| unknown(name))?;
        let entity = &player.living_entity.entity;
        let sneaking = !entity.sneaking.load(std::sync::atomic::Ordering::Relaxed);
        entity.set_sneaking(sneaking);
        sender.send_message(TextComponent::text(format!(
            "{name} is {}sneaking",
            if sneaking { "" } else { "no longer " }
        )));
        Ok(1)
    }
}

struct JumpExecutor;

impl CommandExecutor for JumpExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_NAME)?;
        let player = fake_player::get(name).ok_or_else(|| unknown(name))?;
        if !player.jump_local() {
            return Err(text_error(format!(
                "Fake player {name} cannot jump while airborne"
            )));
        }
        sender.send_message(TextComponent::text(format!("{name} jumped")));
        Ok(1)
    }
}

struct DropExecutor;

impl CommandExecutor for DropExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_NAME)?;
        let player = fake_player::get(name).ok_or_else(|| unknown(name))?;
        player.drop_held_item(false);
        sender.send_message(TextComponent::text(format!("{name} dropped an item")));
        Ok(1)
    }
}

struct MountExecutor;

impl CommandExecutor for MountExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_NAME)?;
        match fake_player::mount(name) {
            Ok(vehicle) => {
                sender.send_message(TextComponent::text(format!("{name} mounted {vehicle}")));
                Ok(1)
            }
            Err(error) => Err(text_error(error)),
        }
    }
}

struct DismountExecutor;

impl CommandExecutor for DismountExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_NAME)?;
        fake_player::dismount(name).map_err(text_error)?;
        sender.send_message(TextComponent::text(format!("{name} dismounted")));
        Ok(1)
    }
}

struct StopExecutor;

impl CommandExecutor for StopExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs<'_>,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_NAME)?;
        fake_player::stop_actions(name).map_err(text_error)?;
        sender.send_message(TextComponent::text(format!("Stopped actions of {name}")));
        Ok(1)
    }
}

fn invalid_value(arg: &str) -> crate::command::dispatcher::CommandError {
    crate::command::dispatcher::CommandError::CommandFailed(TextComponent::text(format!(
        "Invalid value for {arg}"
    )))
}

fn unknown(name: &str) -> crate::command::dispatcher::CommandError {
    crate::command::dispatcher::CommandError::CommandFailed(TextComponent::text(format!(
        "No fake player named {name}"
    )))
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .then(literal("list").execute(ListExecutor))
        .then(
            argument(ARG_NAME, SimpleArgConsumer)
                .then(literal("spawn").execute(SpawnExecutor))
                .then(literal("kill").execute(KillExecutor))
                .then(literal("attack").execute(AttackExecutor))
                .then(literal("sneak").execute(SneakExecutor))
                .then(literal("jump").execute(JumpExecutor))
                .then(literal("drop").execute(DropExecutor))
                .then(literal("mount").execute(MountExecutor))
                .then(literal("dismount").execute(DismountExecutor))
                .then(literal("stop").execute(StopExecutor))
                .then(
                    literal("look").then(
                        argument(ARG_YAW, SimpleArgConsumer)
                            .then(argument(ARG_PITCH, SimpleArgConsumer).execute(LookExecutor)),
                    ),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_player_names_match_vanilla_rules() {
        assert!(fake_player::valid_name("Steve"));
        assert!(fake_player::valid_name("farm_bot_1"));
        assert!(!fake_player::valid_name("a"));
        assert!(!fake_player::valid_name("ab"));
        assert!(!fake_player::valid_name("fake-player"));
        assert!(!fake_player::valid_name("this_name_is_way_too_long"));
        assert!(!fake_player::valid_name("bad name!"));
    }
}
