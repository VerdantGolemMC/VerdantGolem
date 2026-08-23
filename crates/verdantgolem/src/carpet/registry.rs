//! Carpet-style rule catalog: static metadata, runtime values and persistence.
//!
//! 语义与 fabric-carpet 的 `CarpetSettings` 对齐：规则在运行时通过 `/carpet` 修改、
//! 按名称持久化到服务器根目录的 `carpet_rules.json`，未知或缺失的条目回落到默认值。
//! 每条规则都必须在真实游戏逻辑中接线；没有接线的规则不允许出现在目录里。

use std::{
    collections::BTreeMap,
    fmt,
    io::Write as _,
    path::Path,
    path::PathBuf,
    sync::{
        LazyLock, Mutex, OnceLock, RwLock, RwLockReadGuard,
        atomic::{AtomicU64, Ordering},
    },
};

use serde::{Deserialize, Serialize};

/// Rule groups shown by `/carpet list`, aligned with carpet's categories.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuleCategory {
    Tnt,
    Creative,
    Survival,
    Optimization,
    Feature,
}

impl RuleCategory {
    pub const ALL: [Self; 5] = [
        Self::Tnt,
        Self::Creative,
        Self::Survival,
        Self::Optimization,
        Self::Feature,
    ];

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|category| category.name() == name)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tnt => "tnt",
            Self::Creative => "creative",
            Self::Survival => "survival",
            Self::Optimization => "optimization",
            Self::Feature => "feature",
        }
    }
}

impl fmt::Display for RuleCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The value type a rule accepts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ValueKind {
    Bool,
    Int,
    Float,
}

impl ValueKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bool => "boolean",
            Self::Int => "integer",
            Self::Float => "float",
        }
    }
}

/// A typed rule value.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RuleValue {
    Bool(bool),
    Int(i64),
    Float(f64),
}

impl RuleValue {
    #[must_use]
    pub const fn kind(self) -> ValueKind {
        match self {
            Self::Bool(_) => ValueKind::Bool,
            Self::Int(_) => ValueKind::Int,
            Self::Float(_) => ValueKind::Float,
        }
    }

    #[must_use]
    pub const fn as_bool(self) -> bool {
        if let Self::Bool(value) = self {
            value
        } else {
            false
        }
    }

    #[must_use]
    pub const fn as_int(self) -> i64 {
        if let Self::Int(value) = self {
            value
        } else {
            0
        }
    }

    #[must_use]
    pub const fn as_float(self) -> f64 {
        if let Self::Float(value) = self {
            value
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Bool(value) => i64::from(value) as f64,
            Self::Int(value) => value as f64,
            Self::Float(value) => value,
        }
    }
}

impl fmt::Display for RuleValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::Int(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}"),
        }
    }
}

/// Static metadata for one rule.
#[derive(Clone, Copy, Debug)]
pub struct RuleDef {
    pub name: &'static str,
    pub category: RuleCategory,
    pub kind: ValueKind,
    pub default: RuleValue,
    /// Inclusive lower bound for numeric rules.
    pub min: Option<f64>,
    /// Inclusive upper bound for numeric rules.
    pub max: Option<f64>,
    pub desc: &'static str,
}

impl RuleDef {
    fn validate(&self, value: RuleValue) -> Result<(), String> {
        if value.kind() != self.kind {
            return Err(format!(
                "rule {} expects a {} value",
                self.name,
                self.kind.name()
            ));
        }
        if let RuleValue::Float(number) = value
            && !number.is_finite()
        {
            return Err(format!("rule {} requires a finite number", self.name));
        }
        if matches!(self.name, "tntRandomRange" | "hardcodeTNTangle") {
            let number = value.as_float();
            if number != -1.0 && number < 0.0 {
                return Err(format!(
                    "rule {} must be -1 or a non-negative number",
                    self.name
                ));
            }
        }
        if self.name == "hardcodeTNTangle" && value.as_float() >= std::f64::consts::TAU {
            return Err(format!(
                "rule {} must be -1 or in the range [0, 2pi)",
                self.name
            ));
        }
        if let Some(min) = self.min
            && value.as_f64() < min
        {
            return Err(format!("rule {} must be >= {min}", self.name));
        }
        if let Some(max) = self.max
            && value.as_f64() > max
        {
            return Err(format!("rule {} must be <= {max}", self.name));
        }
        Ok(())
    }
}

/// Maps a `ValueKind` unit variant to the field type used in [`RuleValues`].
macro_rules! kind_ty {
    (Bool) => {
        bool
    };
    (Int) => {
        i64
    };
    (Float) => {
        f64
    };
}

/// Declares the rule enum, its metadata, the values struct and field (de)serialization.
macro_rules! carpet_rules {
    ($( $variant:ident => {
        name: $name:literal,
        category: $category:ident,
        kind: $kind:ident,
        default: $default:expr,
        $(min: $min:expr,)?
        $(max: $max:expr,)?
        field: $field:ident,
        desc: $desc:literal,
    }; )*) => {
        /// Every available carpet rule.
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum Rule {
            $($variant,)*
        }

        impl Rule {
            /// All rules, in catalog order.
            pub const ALL: &'static [Rule] = &[$(Rule::$variant,)*];

            #[must_use]
            pub const fn def(self) -> RuleDef {
                match self {
                    $(Rule::$variant => RuleDef {
                        name: $name,
                        category: RuleCategory::$category,
                        kind: ValueKind::$kind,
                        default: $default,
                        min: carpet_rules!(@bound $( $min )?),
                        max: carpet_rules!(@bound $( $max )?),
                        desc: $desc,
                    },)*
                }
            }

            #[must_use]
            pub fn from_name(name: &str) -> Option<Rule> {
                Rule::ALL
                    .iter()
                    .copied()
                    .find(|rule| rule.def().name == name)
            }
        }

        impl fmt::Display for Rule {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.def().name)
            }
        }

        /// Snapshot of all rule values; cheap to copy under the read lock.
        #[derive(Clone, Copy, Debug)]
        pub struct RuleValues {
            $(pub $field: kind_ty!($kind),)*
        }

        impl Default for RuleValues {
            fn default() -> Self {
                Self {
                    $($field: carpet_rules!(@default $kind, $default),)*
                }
            }
        }

        impl RuleValues {
            const fn write(&mut self, rule: Rule, value: RuleValue) {
                match rule {
                    $(Rule::$variant => match value {
                        RuleValue::$kind(v) => {
                            self.$field = v;
                        }
                        _ => {}
                    },)*
                }
            }

            const fn read(&self, rule: Rule) -> RuleValue {
                match rule {
                    $(Rule::$variant => RuleValue::$kind(self.$field),)*
                }
            }
        }
    };

    (@bound) => {
        None
    };
    (@bound $v:expr) => {
        Some($v)
    };
    (@default Bool, $value:expr) => {
        $value.as_bool()
    };
    (@default Int, $value:expr) => {
        $value.as_int()
    };
    (@default Float, $value:expr) => {
        $value.as_float()
    };
}

carpet_rules! {
    ExplosionNoBlockDamage => {
        name: "explosionNoBlockDamage",
        category: Tnt,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: explosion_no_block_damage,
        desc: "Explosions do not destroy blocks.",
    };
    ExplosionNoEntityDamage => {
        name: "explosionNoEntityDamage",
        category: Tnt,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: explosion_no_entity_damage,
        desc: "Explosions do not damage entities.",
    };
    TntPrimerMomentumRemoved => {
        name: "tntPrimerMomentumRemoved",
        category: Tnt,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: tnt_primer_momentum_removed,
        desc: "Primed TNT gets no random horizontal momentum.",
    };
    TntDoNotUpdate => {
        name: "tntDoNotUpdate",
        category: Tnt,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: tnt_do_not_update,
        desc: "TNT placed next to an existing power source does not prime during placement.",
    };
    TntRandomRange => {
        name: "tntRandomRange",
        category: Tnt,
        kind: Float,
        default: RuleValue::Float(-1.0),
        min: -1.0,
        field: tnt_random_range,
        desc: "Fixed TNT explosion ray random factor (-1 keeps vanilla randomness).",
    };
    HardcodeTntAngle => {
        name: "hardcodeTNTangle",
        category: Tnt,
        kind: Float,
        default: RuleValue::Float(-1.0),
        min: -1.0,
        max: std::f64::consts::TAU,
        field: hardcode_tnt_angle,
        desc: "Fixed horizontal launch angle for primed TNT (-1 keeps vanilla randomness).",
    };
    MergeTnt => {
        name: "mergeTNT",
        category: Tnt,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: merge_tnt,
        desc: "Stationary primed TNT entities merge into one to reduce entity load.",
    };
    HopperCounters => {
        name: "hopperCounters",
        category: Feature,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: hopper_counters,
        desc: "Hoppers transferring into wool count and destroy the items (16 channels, /counter).",
    };
    RenewableSponges => {
        name: "renewableSponges",
        category: Feature,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: renewable_sponges,
        desc: "Lightning striking a guardian turns it into an elder guardian.",
    };
    MovableAmethyst => {
        name: "movableAmethyst",
        category: Feature,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: movable_amethyst,
        desc: "Amethyst blocks (including budding) can be moved by pistons.",
    };
    RenewableDeepslate => {
        name: "renewableDeepslate",
        category: Feature,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: renewable_deepslate,
        desc: "Lava meeting water below y=0 produces deepslate instead of cobblestone.",
    };
    RenewableBlackstone => {
        name: "renewableBlackstone",
        category: Feature,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: renewable_blackstone,
        desc: "Lava flowing over blue ice without soul soil forms blackstone instead of nothing.",
    };
    MissingTools => {
        name: "missingTools",
        category: Survival,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: missing_tools,
        desc: "Pickaxes also break glass at pickaxe speed.",
    };
    DesertShrubs => {
        name: "desertShrubs",
        category: Feature,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: desert_shrubs,
        desc: "Saplings in hot, dry biomes wither into dead bushes (unless water is near).",
    };
    RotatorBlock => {
        name: "rotatorBlock",
        category: Feature,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: rotator_block,
        desc: "A dispenser holding a cactus rotates the block it faces counter-clockwise.",
    };
    FillUpdates => {
        name: "fillUpdates",
        category: Creative,
        kind: Bool,
        default: RuleValue::Bool(true),
        field: fill_updates,
        desc: "Blocks placed by /fill, /clone and /setblock cause neighbor updates.",
    };
    FillLimit => {
        name: "fillLimit",
        category: Creative,
        kind: Int,
        default: RuleValue::Int(32_768),
        min: 1.0,
        max: 2_000_000.0,
        field: fill_limit,
        desc: "Maximum volume changed by /fill and /clone in one command.",
    };
    MaxEntityCollisions => {
        name: "maxEntityCollisions",
        category: Optimization,
        kind: Int,
        default: RuleValue::Int(0),
        min: 0.0,
        field: max_entity_collisions,
        desc: "Maximum entity collisions processed per entity per tick (0 = unlimited).",
    };
    MomentumClampThreshold => {
        name: "momentumClampThreshold",
        category: Optimization,
        kind: Float,
        default: RuleValue::Float(0.003),
        min: 0.0,
        max: 1.0,
        field: momentum_clamp_threshold,
        desc: "Momentum below this is zeroed; 0 disables the vanilla clamp.",
    };
    XpNoCooldown => {
        name: "xpNoCooldown",
        category: Survival,
        kind: Bool,
        default: RuleValue::Bool(false),
        field: xp_no_cooldown,
        desc: "Players absorb experience orbs instantly without the pickup delay.",
    };
    MobCapMultiplier => {
        name: "mobCapMultiplier",
        category: Optimization,
        kind: Float,
        default: RuleValue::Float(1.0),
        min: 0.0,
        max: 1_000.0,
        field: mob_cap_multiplier,
        desc: "Multiplier applied to the global mob cap formula.",
    };
    ForceloadLimit => {
        name: "forceloadLimit",
        category: Feature,
        kind: Int,
        default: RuleValue::Int(256),
        min: 1.0,
        field: forceload_limit,
        desc: "Maximum number of chunks force-loaded via /forceload.",
    };
    SpawnChunkRadius => {
        name: "spawnChunkRadius",
        category: Feature,
        kind: Int,
        default: RuleValue::Int(0),
        min: 0.0,
        max: 32.0,
        field: spawn_chunk_radius,
        desc: "Radius of spawn chunks kept active without players nearby (0 = vanilla).",
    };
    CreativePlayersLoadChunks => {
        name: "creativePlayersLoadChunks",
        category: Creative,
        kind: Bool,
        default: RuleValue::Bool(true),
        field: creative_players_load_chunks,
        desc: "Creative-mode players still load chunks; set false to treat them like spectators.",
    };
    PushLimit => {
        name: "pushLimit",
        category: Creative,
        kind: Int,
        default: RuleValue::Int(12),
        min: 1.0,
        max: 1024.0,
        field: push_limit,
        desc: "Maximum number of blocks a piston can push.",
    };
    RailPowerLimit => {
        name: "railPowerLimit",
        category: Creative,
        kind: Int,
        default: RuleValue::Int(9),
        min: 1.0,
        max: 1024.0,
        field: rail_power_limit,
        desc: "Distance powered rails propagate their power along a track.",
    };
    PingPlayerListLimit => {
        name: "pingPlayerListLimit",
        category: Creative,
        kind: Int,
        default: RuleValue::Int(12),
        min: 0.0,
        field: ping_player_list_limit,
        desc: "Maximum number of players included in the status (ping) player sample.",
    };
}

/// Runtime state for all carpet rules, loaded from and persisted to a JSON file.
pub struct CarpetRules {
    values: RwLock<RuleValues>,
    path: OnceLock<PathBuf>,
    persist_lock: Mutex<()>,
}

impl CarpetRules {
    /// The process-wide rule store. Starts with defaults until [`CarpetRules::init`] loads a file.
    #[must_use]
    pub fn global() -> &'static Self {
        static GLOBAL: LazyLock<CarpetRules> = LazyLock::new(|| CarpetRules {
            values: RwLock::new(RuleValues::default()),
            path: OnceLock::new(),
            persist_lock: Mutex::new(()),
        });
        &GLOBAL
    }

    /// Loads persisted rule values from `path` (falling back to defaults for missing
    /// entries) and remembers the path for later [`CarpetRules::set`] writes.
    pub fn init(&self, path: PathBuf) {
        if let Some(existing) = self.path.get()
            && existing != &path
        {
            tracing::error!(
                "Refusing to reinitialize carpet rules from {} because {} is already active",
                path.display(),
                existing.display()
            );
            return;
        }
        let _ = self.path.set(path.clone());

        let mut values = RuleValues::default();
        let should_persist = match std::fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str::<BTreeMap<String, serde_json::Value>>(&raw) {
                Ok(saved) => {
                    for (name, raw_value) in saved {
                        // Entries that fail validation are dropped without affecting other rules.
                        if let Some(rule) = Rule::from_name(&name)
                            && let Ok(value) = Self::value_from_json(rule, &raw_value)
                            && rule.def().validate(value).is_ok()
                        {
                            values.write(rule, value);
                        }
                    }
                    true
                }
                Err(error) => {
                    tracing::error!(
                        "Failed to parse {}: {error}; preserving the damaged file and using defaults",
                        path.display()
                    );
                    false
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(error) => {
                tracing::error!(
                    "Failed to read {}: {error}; preserving the file and using defaults",
                    path.display()
                );
                false
            }
        };

        let _persist_guard = self
            .persist_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut current = self
            .values
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *current = values;
        if should_persist && let Err(error) = self.persist_values(&current) {
            tracing::error!("Failed to persist carpet rules: {error}");
        }
    }

    /// Current values snapshot guard.
    pub fn values(&self) -> RwLockReadGuard<'_, RuleValues> {
        self.values
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn get(&self, rule: Rule) -> RuleValue {
        self.values().read(rule)
    }

    /// Validates `value` against the rule metadata, applies it and persists the store.
    pub fn set(&self, rule: Rule, value: RuleValue) -> Result<(), String> {
        rule.def().validate(value)?;
        let _persist_guard = self
            .persist_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut values = self
            .values
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = values.read(rule);
        values.write(rule, value);
        if let Err(error) = self.persist_values(&values) {
            values.write(rule, previous);
            return Err(error);
        }
        Ok(())
    }

    /// Resets one rule to its default value.
    pub fn reset(&self, rule: Rule) -> Result<(), String> {
        self.set(rule, rule.def().default)
    }

    fn value_from_json(rule: Rule, raw: &serde_json::Value) -> Result<RuleValue, String> {
        let value = match rule.def().kind {
            ValueKind::Bool => raw.as_bool().map(RuleValue::Bool),
            ValueKind::Int => raw.as_i64().map(RuleValue::Int),
            // `as_f64` intentionally accepts both integer and floating-point JSON numbers.
            ValueKind::Float => raw.as_f64().map(RuleValue::Float),
        };
        value.ok_or_else(|| {
            format!(
                "rule {} expects a {} value",
                rule.def().name,
                rule.def().kind.name()
            )
        })
    }

    fn persist_values(&self, values: &RuleValues) -> Result<(), String> {
        let Some(path) = self.path.get() else {
            return Ok(());
        };
        let saved: BTreeMap<String, RuleValue> = Rule::ALL
            .iter()
            .copied()
            .map(|rule| (rule.def().name.to_string(), values.read(rule)))
            .collect();
        let mut json = serde_json::to_vec_pretty(&saved)
            .map_err(|error| format!("failed to serialize carpet rules: {error}"))?;
        json.push(b'\n');
        Self::atomic_write(path, &json)
    }

    fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
        static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("invalid carpet rule path: {}", path.display()))?;
        let temp_id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            temp_id
        ));

        let result = (|| -> std::io::Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(contents)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temp_path, path)
        })();

        if let Err(error) = result {
            let _ = std::fs::remove_file(&temp_path);
            return Err(format!(
                "failed to atomically write {}: {error}",
                path.display()
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_without_path() -> CarpetRules {
        CarpetRules {
            values: RwLock::new(RuleValues::default()),
            path: OnceLock::new(),
            persist_lock: Mutex::new(()),
        }
    }

    #[test]
    fn catalog_is_unique_and_complete() {
        for (index, rule) in Rule::ALL.iter().enumerate() {
            let def = rule.def();
            // Names are unique.
            for other in Rule::ALL.iter().skip(index + 1) {
                assert_ne!(def.name, other.def().name);
            }
            // Defaults satisfy their own bounds and kinds.
            assert!(def.validate(def.default).is_ok(), "{}", def.name);
            assert_eq!(def.default.kind(), def.kind);
            // Every rule resolves back from its name.
            assert_eq!(Rule::from_name(def.name), Some(*rule));
        }
        assert_eq!(Rule::ALL.len(), 27);
    }

    #[test]
    fn set_validates_kind_and_bounds() {
        let rules = rules_without_path();

        let rule = Rule::FillLimit;
        assert!(rules.set(rule, RuleValue::Int(100)).is_ok());
        assert_eq!(rules.get(rule), RuleValue::Int(100));
        assert!(rules.set(rule, RuleValue::Int(0)).is_err(), "below min");
        assert!(
            rules.set(rule, RuleValue::Bool(true)).is_err(),
            "wrong kind"
        );

        let rule = Rule::HardcodeTntAngle;
        assert!(rules.set(rule, RuleValue::Float(1.5)).is_ok());
        assert!(rules.set(rule, RuleValue::Float(7.0)).is_err(), "above max");

        assert!(rules.reset(rule).is_ok());
        assert_eq!(rules.get(rule), rule.def().default);
    }

    #[test]
    fn rejects_non_finite_and_invalid_tnt_float_values() {
        let rules = rules_without_path();

        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                rules
                    .set(Rule::MobCapMultiplier, RuleValue::Float(value))
                    .is_err()
            );
        }
        assert!(
            rules
                .set(Rule::MomentumClampThreshold, RuleValue::Float(1.01))
                .is_err()
        );
        assert!(
            rules
                .set(Rule::MobCapMultiplier, RuleValue::Float(1_001.0))
                .is_err()
        );
        assert!(
            rules
                .set(Rule::TntRandomRange, RuleValue::Float(-0.5))
                .is_err()
        );
        for value in [-0.5, std::f64::consts::TAU] {
            assert!(
                rules
                    .set(Rule::HardcodeTntAngle, RuleValue::Float(value))
                    .is_err()
            );
        }
        assert!(
            rules
                .set(Rule::TntRandomRange, RuleValue::Float(1.5))
                .is_ok()
        );
        assert!(
            rules
                .set(Rule::HardcodeTntAngle, RuleValue::Float(0.0))
                .is_ok()
        );
    }

    #[test]
    fn init_falls_back_to_defaults_for_garbage() {
        let dir = std::env::temp_dir().join(format!("vg-rules-{}", std::process::id()));
        _ = std::fs::create_dir_all(&dir);
        let path = dir.join("carpet_rules.json");
        _ = std::fs::write(
            &path,
            "{\"fillLimit\": 512, \"unknownRule\": true, \"xpNoCooldown\": \"yes\"}",
        );

        let rules = rules_without_path();
        rules.init(path.clone());

        assert_eq!(rules.get(Rule::FillLimit), RuleValue::Int(512));
        assert_eq!(rules.get(Rule::XpNoCooldown), RuleValue::Bool(false));
        assert_eq!(rules.get(Rule::MergeTnt), RuleValue::Bool(false));

        // The persisted file is rewritten without the unknown/invalid entries.
        let raw = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(raw.contains("\"fillLimit\": 512"));
        assert!(!raw.contains("unknownRule"));
        _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn init_accepts_integer_json_for_float_rules() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("carpet_rules.json");
        std::fs::write(&path, r#"{"mobCapMultiplier": 2}"#)?;

        let rules = rules_without_path();
        rules.init(path.clone());

        assert_eq!(rules.get(Rule::MobCapMultiplier), RuleValue::Float(2.0));
        let saved = std::fs::read_to_string(path)?;
        assert!(saved.contains(r#""mobCapMultiplier": 2.0"#));
        Ok(())
    }

    #[test]
    fn malformed_rule_file_is_preserved() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("carpet_rules.json");
        let malformed = "{ definitely not json";
        std::fs::write(&path, malformed)?;

        let rules = rules_without_path();
        rules.init(path.clone());

        assert_eq!(rules.get(Rule::MobCapMultiplier), RuleValue::Float(1.0));
        assert_eq!(std::fs::read_to_string(path)?, malformed);
        Ok(())
    }

    #[test]
    fn failed_persistence_rolls_back_runtime_value() -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let missing_parent = dir.path().join("missing");
        let path = missing_parent.join("carpet_rules.json");
        let rules = rules_without_path();
        rules.init(path);

        let result = rules.set(Rule::FillLimit, RuleValue::Int(100));
        assert!(result.is_err());
        assert_eq!(rules.get(Rule::FillLimit), RuleValue::Int(32_768));
        Ok(())
    }

    #[test]
    fn concurrent_rule_updates_share_one_persisted_snapshot()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("carpet_rules.json");
        let rules = std::sync::Arc::new(rules_without_path());
        rules.init(path.clone());

        let first = rules.clone();
        let first_update =
            std::thread::spawn(move || first.set(Rule::FillLimit, RuleValue::Int(100_000)));
        let second = rules.clone();
        let second_update =
            std::thread::spawn(move || second.set(Rule::MobCapMultiplier, RuleValue::Float(2.0)));
        first_update
            .join()
            .map_err(|_| std::io::Error::other("first update thread panicked"))?
            .map_err(std::io::Error::other)?;
        second_update
            .join()
            .map_err(|_| std::io::Error::other("second update thread panicked"))?
            .map_err(std::io::Error::other)?;

        let saved: BTreeMap<String, serde_json::Value> =
            serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(saved.get("fillLimit"), Some(&serde_json::json!(100_000)));
        assert_eq!(saved.get("mobCapMultiplier"), Some(&serde_json::json!(2.0)));
        Ok(())
    }
}
