use std::fmt::Write as _;

use crate::TextComponent;
use crate::carpet::counters::{self, CounterReport, TimeBasis};

use crate::command::args::ConsumedArgs;
use crate::command::tree::CommandTree;
use crate::command::tree::builder::literal;
use crate::command::{CommandExecutor, CommandResult, CommandSender};

const NAMES: [&str; 1] = ["counter"];

const DESCRIPTION: &str = "Reads or resets wool hopper counts and production rates.";

/// How many items to show per channel before truncating.
const MAX_ITEMS_SHOWN: usize = 10;

fn command_count(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn rate_text(report: &CounterReport, count: u64, basis: TimeBasis) -> String {
    report.items_per_hour(count, basis).map_or_else(
        || "rate pending (no elapsed time)".to_string(),
        |rate| format!("{rate:.1} items/h"),
    )
}

fn measurement_text(report: &CounterReport, basis: TimeBasis) -> String {
    let seconds = report.elapsed(basis).as_secs_f64();
    match basis {
        TimeBasis::GameTime => format!(
            "{seconds:.2}s game-time ({} ticks @ 20 TPS)",
            report.elapsed_ticks
        ),
        TimeBasis::RealTime => format!("{seconds:.2}s real-time (includes pauses)"),
    }
}

fn channel_summary(name: &str, report: &CounterReport, basis: TimeBasis) -> String {
    if !report.started {
        return format!("{name}: 0 items total; not started (waiting for first item)");
    }
    format!(
        "{name}: {} items total; {}; {}",
        report.total,
        measurement_text(report, basis),
        rate_text(report, report.total, basis)
    )
}

fn channel_report(name: &str, report: &CounterReport, basis: TimeBasis) -> String {
    let mut message = channel_summary(name, report, basis);
    for (item, count) in report.items.iter().take(MAX_ITEMS_SHOWN) {
        let _ = write!(
            message,
            "\n{item}: {count} items ({})",
            rate_text(report, *count, basis)
        );
    }
    if report.items.len() > MAX_ITEMS_SHOWN {
        let _ = write!(
            message,
            "\n... and {} more item types",
            report.items.len() - MAX_ITEMS_SHOWN
        );
    }
    message
}

fn all_channels_report(reports: &[CounterReport; 16], basis: TimeBasis) -> String {
    let mut summary = String::from(match basis {
        TimeBasis::GameTime => "Hopper counters (game-time at 20 TPS, excluding frozen time):",
        TimeBasis::RealTime => "Hopper counters (real-time, including pauses and lag):",
    });
    let mut any_started = false;
    for (name, report) in counters::CHANNEL_NAMES.iter().zip(reports) {
        if report.started {
            any_started = true;
            let _ = write!(summary, "\n{}", channel_summary(name, report, basis));
        }
    }
    if !any_started {
        summary.push_str(
            "\nNo items counted yet. Counting starts on the first item or /counter reset.",
        );
    }
    summary
}

struct AllChannelsExecutor(TimeBasis);

impl CommandExecutor for AllChannelsExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        _args: &ConsumedArgs,
    ) -> CommandResult {
        sender.send_message(TextComponent::text(all_channels_report(
            &counters::reports(),
            self.0,
        )));

        Ok(1)
    }
}

struct ChannelExecutor(usize, TimeBasis);

impl CommandExecutor for ChannelExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        _args: &ConsumedArgs,
    ) -> CommandResult {
        let report = counters::report(self.0);
        let name = counters::CHANNEL_NAMES[self.0];

        sender.send_message(TextComponent::text(channel_report(name, &report, self.1)));

        Ok(command_count(report.total))
    }
}

struct ResetExecutor(Option<usize>);

impl CommandExecutor for ResetExecutor {
    fn execute(
        &self,
        sender: &CommandSender,
        _server: &crate::server::Server,
        _args: &ConsumedArgs,
    ) -> CommandResult {
        counters::reset(self.0);
        let scope = self.0.map_or_else(
            || "all channels".to_string(),
            |index| counters::CHANNEL_NAMES[index].to_string(),
        );
        sender.send_message(TextComponent::text(format!(
            "Reset hopper counter for {scope}; new game-time and real-time measurements started"
        )));

        Ok(1)
    }
}

pub fn init_command_tree() -> CommandTree {
    let mut command_tree = CommandTree::new(NAMES, DESCRIPTION)
        .execute(AllChannelsExecutor(TimeBasis::GameTime))
        .then(literal("realtime").execute(AllChannelsExecutor(TimeBasis::RealTime)))
        .then(literal("reset").execute(ResetExecutor(None)));
    for (index, name) in counters::CHANNEL_NAMES.iter().enumerate() {
        command_tree = command_tree.then(
            literal(*name)
                .execute(ChannelExecutor(index, TimeBasis::GameTime))
                .then(literal("realtime").execute(ChannelExecutor(index, TimeBasis::RealTime)))
                .then(literal("reset").execute(ResetExecutor(Some(index)))),
        );
    }
    command_tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::tree::NodeType;
    use std::time::Duration;

    #[test]
    fn command_result_saturates_large_totals() {
        assert_eq!(command_count(u64::MAX), i32::MAX);
        assert_eq!(command_count(42), 42);
    }

    #[test]
    fn command_tree_keeps_old_syntax_and_adds_both_realtime_forms() {
        let tree = init_command_tree();
        let paths: Vec<_> = tree
            .iter_paths()
            .map(|path| {
                path.iter()
                    .filter_map(|index| match &tree.nodes[*index].node_type {
                        NodeType::Literal { string } => Some(string.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        for path in ["", "reset", "realtime"] {
            assert!(paths.iter().any(|candidate| candidate == path));
        }
        for name in counters::CHANNEL_NAMES {
            for suffix in ["", " reset", " realtime"] {
                assert!(paths.contains(&format!("{name}{suffix}")));
            }
        }
        assert_eq!(paths.len(), 3 + 16 * 3);
    }

    #[test]
    fn reports_show_per_item_rates_and_distinguish_the_clocks() {
        let report = CounterReport {
            items: vec![
                ("minecraft:stone".to_string(), 40),
                ("minecraft:dirt".to_string(), 20),
            ],
            total: 60,
            started: true,
            elapsed_ticks: 1200,
            elapsed_realtime: Duration::from_mins(2),
        };
        let game = channel_report("white", &report, TimeBasis::GameTime);
        assert!(
            game.contains("60 items total; 60.00s game-time (1200 ticks @ 20 TPS); 3600.0 items/h")
        );
        assert!(game.contains("minecraft:stone: 40 items (2400.0 items/h)"));
        assert!(game.contains("minecraft:dirt: 20 items (1200.0 items/h)"));
        let real = channel_report("white", &report, TimeBasis::RealTime);
        assert!(real.contains("120.00s real-time (includes pauses); 1800.0 items/h"));
        assert!(real.contains("minecraft:stone: 40 items (1200.0 items/h)"));
    }

    #[test]
    fn empty_and_zero_time_reports_do_not_invent_a_rate() {
        let mut report = CounterReport::default();
        assert!(
            channel_report("red", &report, TimeBasis::GameTime).contains("waiting for first item")
        );
        report.started = true;
        for basis in [TimeBasis::GameTime, TimeBasis::RealTime] {
            let text = channel_report("red", &report, basis);
            assert!(text.contains("rate pending (no elapsed time)"));
            assert!(!text.contains("NaN"));
            assert!(!text.contains("inf"));
        }
        report.elapsed_ticks = 20;
        assert!(channel_report("red", &report, TimeBasis::GameTime).contains("0.0 items/h"));
    }

    #[test]
    fn truncated_items_still_contribute_to_the_displayed_total() {
        let report = CounterReport {
            items: (0..12)
                .map(|index| (format!("minecraft:item_{index}"), 1))
                .collect(),
            total: 12,
            started: true,
            elapsed_ticks: 72_000,
            elapsed_realtime: Duration::from_hours(1),
        };
        let text = channel_report("blue", &report, TimeBasis::GameTime);
        assert!(text.contains("12 items total"));
        assert!(text.contains("12.0 items/h"));
        assert!(text.contains("... and 2 more item types"));
        assert!(!text.contains("minecraft:item_10:"));
    }

    #[test]
    fn all_channels_includes_reset_empty_sessions() {
        let mut reports = std::array::from_fn(|_| CounterReport::default());
        assert!(
            all_channels_report(&reports, TimeBasis::GameTime).contains("No items counted yet")
        );
        reports[0].started = true;
        reports[0].elapsed_ticks = 20;
        let text = all_channels_report(&reports, TimeBasis::GameTime);
        assert!(text.contains("white: 0 items total"));
        assert!(text.contains("0.0 items/h"));
        assert!(!text.contains("orange:"));
    }
}
