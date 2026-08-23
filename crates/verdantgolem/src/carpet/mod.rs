//! Carpet-style technical-survival features for VerdantGolem.
//!
//! The rule registry (`registry`) mirrors fabric-carpet's `CarpetSettings` semantics:
//! runtime-changeable via `/carpet`, persisted per rule name to `carpet_rules.json`,
//! defaults for everything unknown or missing. The hopper counters (`counters`) back
//! the `hopperCounters` rule and the `/counter` command.

pub mod counters;
pub mod fake_player;
pub mod loggers;
pub mod registry;
pub mod spawn_tracking;

use std::sync::RwLockReadGuard;

pub use registry::{CarpetRules, Rule, RuleCategory, RuleValue, ValueKind};

/// Shortcut for [`CarpetRules::global`] values used across gameplay code.
pub fn values() -> RwLockReadGuard<'static, registry::RuleValues> {
    CarpetRules::global().values()
}

/// Initializes the rule store from `<server_root>/carpet_rules.json`.
pub fn init(server_root: &std::path::Path) {
    CarpetRules::global().init(server_root.join("carpet_rules.json"));
}
