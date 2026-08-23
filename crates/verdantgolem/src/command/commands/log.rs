use crate::TextComponent;
use crate::carpet::loggers::{self, Logger};

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::literal;
use crate::command::{CommandExecutor, CommandResult, CommandSender};

const NAMES: [&str; 1] = ["log"];

const DESCRIPTION: &str = "Subscribes to repeating action-bar readouts.";

struct ToggleExecutor(Logger);

impl CommandExecutor for ToggleExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a crate::server::Server,
        _args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let Some(player) = sender.as_player() else {
                return Err(failed("loggers are only available to players".to_string()));
            };
            let name = match self.0 {
                Logger::Tps => "tps",
                Logger::MobCaps => "mobcaps",
            };
            let enabled = loggers::toggle(self.0, &player);
            sender
                .send_message(TextComponent::text(format!(
                    "Logger {name} {}",
                    if enabled { "enabled" } else { "disabled" }
                )))
                .await;
            Ok(i32::from(enabled))
        })
    }
}

struct ClearExecutor;

impl CommandExecutor for ClearExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a crate::server::Server,
        _args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let Some(player) = sender.as_player() else {
                return Err(failed("loggers are only available to players".to_string()));
            };
            loggers::clear(&player);
            sender
                .send_message(TextComponent::text("Disabled all loggers"))
                .await;
            Ok(1)
        })
    }
}

fn failed(message: String) -> crate::command::dispatcher::CommandError {
    crate::command::dispatcher::CommandError::CommandFailed(TextComponent::text(message))
}

pub fn init_command_tree() -> CommandTree {
    CommandTree::new(NAMES, DESCRIPTION)
        .then(literal("tps").execute(ToggleExecutor(Logger::Tps)))
        .then(literal("mobcaps").execute(ToggleExecutor(Logger::MobCaps)))
        .then(literal("stop").execute(ClearExecutor))
}
