use crate::plugin::loader::wasm::wasm_host::{
    state::PluginHostState, wit::v0_1::pumpkin::plugin::status_effect,
};

impl status_effect::Host for PluginHostState {}

#[must_use]
#[allow(clippy::too_many_lines)]
pub const fn from_wasm_status_effect_type(
    t: status_effect::StatusEffectType,
) -> verdantgolem_data::status_effect::EffectType {
    match t {
        status_effect::StatusEffectType::Speed => {
            verdantgolem_data::status_effect::EffectType::Speed
        }
        status_effect::StatusEffectType::Slowness => {
            verdantgolem_data::status_effect::EffectType::Slowness
        }
        status_effect::StatusEffectType::Haste => {
            verdantgolem_data::status_effect::EffectType::Haste
        }
        status_effect::StatusEffectType::MiningFatigue => {
            verdantgolem_data::status_effect::EffectType::MiningFatigue
        }
        status_effect::StatusEffectType::Strength => {
            verdantgolem_data::status_effect::EffectType::Strength
        }
        status_effect::StatusEffectType::InstantHealth => {
            verdantgolem_data::status_effect::EffectType::InstantHealth
        }
        status_effect::StatusEffectType::InstantDamage => {
            verdantgolem_data::status_effect::EffectType::InstantDamage
        }
        status_effect::StatusEffectType::JumpBoost => {
            verdantgolem_data::status_effect::EffectType::JumpBoost
        }
        status_effect::StatusEffectType::Nausea => {
            verdantgolem_data::status_effect::EffectType::Nausea
        }
        status_effect::StatusEffectType::Regeneration => {
            verdantgolem_data::status_effect::EffectType::Regeneration
        }
        status_effect::StatusEffectType::Resistance => {
            verdantgolem_data::status_effect::EffectType::Resistance
        }
        status_effect::StatusEffectType::FireResistance => {
            verdantgolem_data::status_effect::EffectType::FireResistance
        }
        status_effect::StatusEffectType::WaterBreathing => {
            verdantgolem_data::status_effect::EffectType::WaterBreathing
        }
        status_effect::StatusEffectType::Invisibility => {
            verdantgolem_data::status_effect::EffectType::Invisibility
        }
        status_effect::StatusEffectType::Blindness => {
            verdantgolem_data::status_effect::EffectType::Blindness
        }
        status_effect::StatusEffectType::NightVision => {
            verdantgolem_data::status_effect::EffectType::NightVision
        }
        status_effect::StatusEffectType::Hunger => {
            verdantgolem_data::status_effect::EffectType::Hunger
        }
        status_effect::StatusEffectType::Weakness => {
            verdantgolem_data::status_effect::EffectType::Weakness
        }
        status_effect::StatusEffectType::Poison => {
            verdantgolem_data::status_effect::EffectType::Poison
        }
        status_effect::StatusEffectType::Wither => {
            verdantgolem_data::status_effect::EffectType::Wither
        }
        status_effect::StatusEffectType::HealthBoost => {
            verdantgolem_data::status_effect::EffectType::HealthBoost
        }
        status_effect::StatusEffectType::Absorption => {
            verdantgolem_data::status_effect::EffectType::Absorption
        }
        status_effect::StatusEffectType::Saturation => {
            verdantgolem_data::status_effect::EffectType::Saturation
        }
        status_effect::StatusEffectType::Glowing => {
            verdantgolem_data::status_effect::EffectType::Glowing
        }
        status_effect::StatusEffectType::Levitation => {
            verdantgolem_data::status_effect::EffectType::Levitation
        }
        status_effect::StatusEffectType::Luck => verdantgolem_data::status_effect::EffectType::Luck,
        status_effect::StatusEffectType::Unluck => {
            verdantgolem_data::status_effect::EffectType::Unluck
        }
        status_effect::StatusEffectType::SlowFalling => {
            verdantgolem_data::status_effect::EffectType::SlowFalling
        }
        status_effect::StatusEffectType::ConduitPower => {
            verdantgolem_data::status_effect::EffectType::ConduitPower
        }
        status_effect::StatusEffectType::DolphinsGrace => {
            verdantgolem_data::status_effect::EffectType::DolphinsGrace
        }
        status_effect::StatusEffectType::BadOmen => {
            verdantgolem_data::status_effect::EffectType::BadOmen
        }
        status_effect::StatusEffectType::HeroOfTheVillage => {
            verdantgolem_data::status_effect::EffectType::HeroOfTheVillage
        }
        status_effect::StatusEffectType::Darkness => {
            verdantgolem_data::status_effect::EffectType::Darkness
        }
        status_effect::StatusEffectType::TrialOmen => {
            verdantgolem_data::status_effect::EffectType::TrialOmen
        }
        status_effect::StatusEffectType::RaidOmen => {
            verdantgolem_data::status_effect::EffectType::RaidOmen
        }
        status_effect::StatusEffectType::WindCharged => {
            verdantgolem_data::status_effect::EffectType::WindCharged
        }
        status_effect::StatusEffectType::Weaving => {
            verdantgolem_data::status_effect::EffectType::Weaving
        }
        status_effect::StatusEffectType::Oozing => {
            verdantgolem_data::status_effect::EffectType::Oozing
        }
        status_effect::StatusEffectType::Infested => {
            verdantgolem_data::status_effect::EffectType::Infested
        }
    }
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub const fn to_wasm_status_effect_type(
    t: verdantgolem_data::status_effect::EffectType,
) -> status_effect::StatusEffectType {
    match t {
        verdantgolem_data::status_effect::EffectType::Speed => {
            status_effect::StatusEffectType::Speed
        }
        verdantgolem_data::status_effect::EffectType::Slowness => {
            status_effect::StatusEffectType::Slowness
        }
        verdantgolem_data::status_effect::EffectType::Haste => {
            status_effect::StatusEffectType::Haste
        }
        verdantgolem_data::status_effect::EffectType::MiningFatigue => {
            status_effect::StatusEffectType::MiningFatigue
        }
        verdantgolem_data::status_effect::EffectType::Strength => {
            status_effect::StatusEffectType::Strength
        }
        verdantgolem_data::status_effect::EffectType::InstantHealth => {
            status_effect::StatusEffectType::InstantHealth
        }
        verdantgolem_data::status_effect::EffectType::InstantDamage => {
            status_effect::StatusEffectType::InstantDamage
        }
        verdantgolem_data::status_effect::EffectType::JumpBoost => {
            status_effect::StatusEffectType::JumpBoost
        }
        verdantgolem_data::status_effect::EffectType::Nausea => {
            status_effect::StatusEffectType::Nausea
        }
        verdantgolem_data::status_effect::EffectType::Regeneration => {
            status_effect::StatusEffectType::Regeneration
        }
        verdantgolem_data::status_effect::EffectType::Resistance => {
            status_effect::StatusEffectType::Resistance
        }
        verdantgolem_data::status_effect::EffectType::FireResistance => {
            status_effect::StatusEffectType::FireResistance
        }
        verdantgolem_data::status_effect::EffectType::WaterBreathing => {
            status_effect::StatusEffectType::WaterBreathing
        }
        verdantgolem_data::status_effect::EffectType::Invisibility => {
            status_effect::StatusEffectType::Invisibility
        }
        verdantgolem_data::status_effect::EffectType::Blindness => {
            status_effect::StatusEffectType::Blindness
        }
        verdantgolem_data::status_effect::EffectType::NightVision => {
            status_effect::StatusEffectType::NightVision
        }
        verdantgolem_data::status_effect::EffectType::Hunger => {
            status_effect::StatusEffectType::Hunger
        }
        verdantgolem_data::status_effect::EffectType::Weakness => {
            status_effect::StatusEffectType::Weakness
        }
        verdantgolem_data::status_effect::EffectType::Poison => {
            status_effect::StatusEffectType::Poison
        }
        verdantgolem_data::status_effect::EffectType::Wither => {
            status_effect::StatusEffectType::Wither
        }
        verdantgolem_data::status_effect::EffectType::HealthBoost => {
            status_effect::StatusEffectType::HealthBoost
        }
        verdantgolem_data::status_effect::EffectType::Absorption => {
            status_effect::StatusEffectType::Absorption
        }
        verdantgolem_data::status_effect::EffectType::Saturation => {
            status_effect::StatusEffectType::Saturation
        }
        verdantgolem_data::status_effect::EffectType::Glowing => {
            status_effect::StatusEffectType::Glowing
        }
        verdantgolem_data::status_effect::EffectType::Levitation => {
            status_effect::StatusEffectType::Levitation
        }
        verdantgolem_data::status_effect::EffectType::Luck => status_effect::StatusEffectType::Luck,
        verdantgolem_data::status_effect::EffectType::Unluck => {
            status_effect::StatusEffectType::Unluck
        }
        verdantgolem_data::status_effect::EffectType::SlowFalling => {
            status_effect::StatusEffectType::SlowFalling
        }
        verdantgolem_data::status_effect::EffectType::ConduitPower => {
            status_effect::StatusEffectType::ConduitPower
        }
        verdantgolem_data::status_effect::EffectType::DolphinsGrace => {
            status_effect::StatusEffectType::DolphinsGrace
        }
        verdantgolem_data::status_effect::EffectType::BadOmen => {
            status_effect::StatusEffectType::BadOmen
        }
        verdantgolem_data::status_effect::EffectType::HeroOfTheVillage => {
            status_effect::StatusEffectType::HeroOfTheVillage
        }
        verdantgolem_data::status_effect::EffectType::Darkness => {
            status_effect::StatusEffectType::Darkness
        }
        verdantgolem_data::status_effect::EffectType::TrialOmen => {
            status_effect::StatusEffectType::TrialOmen
        }
        verdantgolem_data::status_effect::EffectType::RaidOmen => {
            status_effect::StatusEffectType::RaidOmen
        }
        verdantgolem_data::status_effect::EffectType::WindCharged => {
            status_effect::StatusEffectType::WindCharged
        }
        verdantgolem_data::status_effect::EffectType::Weaving => {
            status_effect::StatusEffectType::Weaving
        }
        verdantgolem_data::status_effect::EffectType::Oozing => {
            status_effect::StatusEffectType::Oozing
        }
        verdantgolem_data::status_effect::EffectType::Infested => {
            status_effect::StatusEffectType::Infested
        }
    }
}

#[must_use]
pub fn to_wasm_status_effect_instance(
    effect: &verdantgolem_data::potion::Effect,
) -> Option<status_effect::StatusEffectInstance> {
    let name = effect
        .effect_type
        .minecraft_name
        .strip_prefix("minecraft:")
        .unwrap_or(effect.effect_type.minecraft_name);
    let effect_type_enum = verdantgolem_data::status_effect::EffectType::from_name(name)?;
    let wasm_type = to_wasm_status_effect_type(effect_type_enum);
    Some(status_effect::StatusEffectInstance {
        effect_type: wasm_type,
        duration: u32::try_from(effect.duration).unwrap_or(0),
        amplifier: effect.amplifier,
        ambient: effect.ambient,
        show_particles: effect.show_particles,
        show_icon: effect.show_icon,
    })
}
