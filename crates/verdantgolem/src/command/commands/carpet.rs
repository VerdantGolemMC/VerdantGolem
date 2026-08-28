use crate::TextComponent;
use crate::carpet::{CarpetRules, Rule, RuleCategory, ValueKind, registry::RuleValue};

use crate::command::args::FindArg;
use crate::command::args::bool::BoolArgConsumer;
use crate::command::args::simple::SimpleArgConsumer;

use crate::command::args::ConsumedArgs;
use crate::command::dispatcher::CommandError;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::{argument, literal};
use crate::command::{CommandExecutor, CommandResult, CommandSender};

const NAMES: [&str; 2] = ["carpet", "vgcarpet"];

const DESCRIPTION: &str = "Manages VerdantGolem carpet rules.";

const ARG_VALUE: &str = "value";
const ARG_RULE: &str = "rule";
const ARG_CATEGORY: &str = "category";

struct RootExecutor;

impl CommandExecutor for RootExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        _args: &ConsumedArgs,
    ) -> CommandResult {
        let rules = CarpetRules::global();
        let total = Rule::ALL.len();
        let changed = Rule::ALL
            .iter()
            .copied()
            .filter(|rule| rules.get(*rule) != rule.def().default)
            .count();

        sender.send_message(TextComponent::text(format!(
            "{total} carpet rules available, {changed} changed from default. \
             Use /carpet list [category], /carpet <rule> [value] or /carpet default <rule>."
        )));

        Ok(changed as i32)
    }
}

fn list_rules(rules: &CarpetRules, category: Option<RuleCategory>) -> String {
    Rule::ALL
        .iter()
        .copied()
        .filter(|rule| category.is_none_or(|wanted| rule.def().category == wanted))
        .map(|rule| {
            let value = rules.get(rule);
            if value == rule.def().default {
                rule.def().name.to_string()
            } else {
                format!("{} ({})", rule.def().name, value)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

struct ListExecutor;

impl CommandExecutor for ListExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs,
    ) -> CommandResult {
        let category = if args.contains_key(ARG_CATEGORY) {
            let name = SimpleArgConsumer::find_arg(args, ARG_CATEGORY)?;
            Some(
                parse_category(name)
                    .map_err(|error| CommandError::CommandFailed(TextComponent::text(error)))?,
            )
        } else {
            None
        };
        let heading = category.map_or_else(
            || "All carpet rules".to_string(),
            |category| format!("Carpet rules in category {category}"),
        );
        sender.send_message(TextComponent::text(format!(
            "{heading}:\n{}",
            list_rules(CarpetRules::global(), category)
        )));

        Ok(1)
    }
}

struct QueryExecutor(Rule);

impl CommandExecutor for QueryExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        _args: &ConsumedArgs,
    ) -> CommandResult {
        let def = self.0.def();
        let value = CarpetRules::global().get(self.0);
        sender.send_message(TextComponent::text(format!(
            "{}: {} (default {})\n{}",
            def.name, value, def.default, def.desc
        )));

        Ok(1)
    }
}

struct SetExecutor(Rule);

impl CommandExecutor for SetExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs,
    ) -> CommandResult {
        let def = self.0.def();
        let raw = match def.kind {
            ValueKind::Bool => BoolArgConsumer::find_arg(args, ARG_VALUE)?.to_string(),
            ValueKind::Int | ValueKind::Float => {
                SimpleArgConsumer::find_arg(args, ARG_VALUE)?.to_string()
            }
        };
        let value = parse_value(def.kind, &raw);

        match value.and_then(|value| CarpetRules::global().set(self.0, value)) {
            Ok(()) => {
                sender.send_message(TextComponent::text(format!(
                    "Set rule {} to {}",
                    def.name,
                    CarpetRules::global().get(self.0)
                )));
                Ok(1)
            }
            Err(error) => Err(CommandError::CommandFailed(TextComponent::text(format!(
                "Failed to set {}: {error}",
                def.name
            )))),
        }
    }
}

struct ResetExecutor;

impl CommandExecutor for ResetExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        args: &ConsumedArgs,
    ) -> CommandResult {
        let name = SimpleArgConsumer::find_arg(args, ARG_RULE)?;
        let Some(rule) = Rule::from_name(name) else {
            return Err(CommandError::CommandFailed(TextComponent::text(format!(
                "Unknown rule: {name}"
            ))));
        };

        let rules = CarpetRules::global();
        if let Err(error) = rules.reset(rule) {
            return Err(CommandError::CommandFailed(TextComponent::text(format!(
                "Failed to reset {}: {error}",
                rule.def().name
            ))));
        }
        sender.send_message(TextComponent::text(format!(
            "Reset rule {} to default {}",
            rule.def().name,
            rules.get(rule)
        )));

        Ok(1)
    }
}

/// Parses user input into a [`RuleValue`] of the expected kind.
pub fn parse_value(kind: ValueKind, raw: &str) -> Result<RuleValue, String> {
    match kind {
        ValueKind::Bool => match raw {
            "true" => Ok(RuleValue::Bool(true)),
            "false" => Ok(RuleValue::Bool(false)),
            _ => Err(format!("expected true or false, got {raw}")),
        },
        ValueKind::Int => raw
            .parse::<i64>()
            .map(RuleValue::Int)
            .map_err(|_| format!("expected an integer, got {raw}")),
        ValueKind::Float => {
            let value = raw
                .parse::<f64>()
                .map_err(|_| format!("expected a number, got {raw}"))?;
            if !value.is_finite() {
                return Err(format!("expected a finite number, got {raw}"));
            }
            Ok(RuleValue::Float(value))
        }
    }
}

fn parse_category(raw: &str) -> Result<RuleCategory, String> {
    RuleCategory::from_name(raw).ok_or_else(|| format!("Unknown carpet rule category: {raw}"))
}

pub fn init_command_tree() -> CommandTree {
    let mut command_tree = CommandTree::new(NAMES, DESCRIPTION).execute(RootExecutor);
    command_tree = command_tree
        .then(
            literal("list")
                .execute(ListExecutor)
                .then(argument(ARG_CATEGORY, SimpleArgConsumer).execute(ListExecutor)),
        )
        .then(
            literal("default").then(argument(ARG_RULE, SimpleArgConsumer).execute(ResetExecutor)),
        );
    for rule in Rule::ALL {
        let value_arg = match rule.def().kind {
            ValueKind::Bool => argument(ARG_VALUE, BoolArgConsumer),
            ValueKind::Int | ValueKind::Float => argument(ARG_VALUE, SimpleArgConsumer),
        };
        command_tree = command_tree.then(
            literal(rule.def().name)
                .execute(QueryExecutor(*rule))
                .then(value_arg.execute(SetExecutor(*rule))),
        );
    }
    command_tree
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_value_matches_kind() {
        assert_eq!(
            parse_value(ValueKind::Bool, "true"),
            Ok(RuleValue::Bool(true))
        );
        assert!(parse_value(ValueKind::Bool, "yes").is_err());
        assert_eq!(parse_value(ValueKind::Int, "-3"), Ok(RuleValue::Int(-3)));
        assert!(parse_value(ValueKind::Int, "1.5").is_err());
        assert_eq!(
            parse_value(ValueKind::Float, "1.5"),
            Ok(RuleValue::Float(1.5))
        );
        for value in ["NaN", "inf", "-inf", "1e400"] {
            assert!(parse_value(ValueKind::Float, value).is_err());
        }
    }

    #[test]
    fn categories_reject_unknown_names() {
        assert_eq!(parse_category("feature"), Ok(RuleCategory::Feature));
        assert!(parse_category("features").is_err());
    }
}
