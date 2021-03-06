use crate::{combat, comp};
use serde::{Deserialize, Serialize};

/// An effect that may be applied to an entity
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Effect {
    Health(comp::HealthChange),
    PoiseChange(comp::PoiseChange),
    Damage(combat::Damage),
    Buff(BuffEffect),
}

/// A buff that may be applied to an entity
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BuffEffect {
    pub kind: comp::BuffKind,
    pub data: comp::BuffData,
    pub cat_ids: Vec<comp::BuffCategory>,
}

impl Effect {
    pub fn info(&self) -> String {
        match self {
            Effect::Health(c) => format!("{:+} health", c.amount),
            Effect::PoiseChange(c) => format!("{:+} poise", c.amount),
            Effect::Damage(d) => format!("{:+}", d.value),
            Effect::Buff(e) => format!("{:?} buff", e),
        }
    }

    pub fn modify_strength(&mut self, modifier: f32) {
        match self {
            Effect::Health(change) => {
                change.amount = (change.amount as f32 * modifier) as i32;
            },
            Effect::PoiseChange(change) => {
                change.amount = (change.amount as f32 * modifier) as i32;
            },
            Effect::Damage(damage) => {
                damage.interpolate_damage(modifier, 0.0);
            },
            Effect::Buff(effect) => {
                effect.data.strength *= modifier;
            },
        }
    }
}
