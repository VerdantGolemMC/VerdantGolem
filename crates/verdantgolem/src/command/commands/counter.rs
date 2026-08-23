use std::fmt::Write as _;

use crate::TextComponent;
use crate::carpet::counters;

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::literal;
use crate::command::{CommandExecutor, CommandResult, CommandSender};

const NAMES: [&str; 1] = ["counter"];

const DESCRIPTION: &str = "Reads or resets the wool hopper counters.";

/// How many items to show per channel before truncating.
const MAX_ITEMS_SHOWN: usize = 10;

fn command_count(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn item_total(items: &[(String, u64)]) -> u64 {
    items
        .iter()
        .fold(0u64, |sum, (_, count)| sum.saturating_add(*count))
}

struct AllChannelsExecutor;

impl CommandExecutor for AllChannelsExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a crate::server::Server,
        _args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let mut summary = String::from("Hopper counters:");
            for (index, name) in counters::CHANNEL_NAMES.iter().enumerate() {
                let total = item_total(&counters::snapshot(index));
                if total > 0 {
                    let _ = write!(summary, "\n{name}: {total} items");
                }
            }
            sender.send_message(TextComponent::text(summary)).await;

            Ok(1)
        })
    }
}

struct ChannelExecutor(usize);

impl CommandExecutor for ChannelExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a crate::server::Server,
        _args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            let items = counters::snapshot(self.0);
            let total = item_total(&items);
            let name = counters::CHANNEL_NAMES[self.0];

            let mut message = format!("{name}: {total} items total");
            for (item, count) in items.iter().take(MAX_ITEMS_SHOWN) {
                let _ = write!(message, "\n{item}: {count}");
            }
            if items.len() > MAX_ITEMS_SHOWN {
                let _ = write!(
                    message,
                    "\n... and {} more item types",
                    items.len() - MAX_ITEMS_SHOWN
                );
            }

            sender.send_message(TextComponent::text(message)).await;

            Ok(command_count(total))
        })
    }
}

struct ResetExecutor(Option<usize>);

impl CommandExecutor for ResetExecutor {
    fn execute<'a>(
        &'a self,
        sender: &'a CommandSender,
        _server: &'a crate::server::Server,
        _args: &'a ConsumedArgs<'a>,
    ) -> CommandResult<'a> {
        Box::pin(async move {
            counters::reset(self.0);
            let scope = self.0.map_or_else(
                || "all channels".to_string(),
                |index| counters::CHANNEL_NAMES[index].to_string(),
            );
            sender
                .send_message(TextComponent::text(format!(
                    "Reset hopper counter for {scope}"
                )))
                .await;

            Ok(1)
        })
    }
}

pub fn init_command_tree() -> CommandTree {
    let mut command_tree = CommandTree::new(NAMES, DESCRIPTION)
        .execute(AllChannelsExecutor)
        .then(literal("reset").execute(ResetExecutor(None)));
    for (index, name) in counters::CHANNEL_NAMES.iter().enumerate() {
        command_tree = command_tree.then(
            literal(*name)
                .execute(ChannelExecutor(index))
                .then(literal("reset").execute(ResetExecutor(Some(index)))),
        );
    }
    command_tree
}

#[cfg(test)]
mod tests {
    use super::{command_count, item_total};

    #[test]
    fn command_result_saturates_large_totals() {
        assert_eq!(command_count(u64::MAX), i32::MAX);
        assert_eq!(command_count(42), 42);
    }

    #[test]
    fn displayed_total_saturates() {
        let items = vec![("a".to_string(), u64::MAX), ("b".to_string(), 1)];
        assert_eq!(item_total(&items), u64::MAX);
    }
}
