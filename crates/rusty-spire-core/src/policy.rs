use serde::{Deserialize, Serialize};

use crate::simulator::SimulatorError;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyKind {
    #[default]
    MinimizeHpLoss,
}

impl PolicyKind {
    pub fn validate(self) -> Result<(), SimulatorError> {
        match self {
            Self::MinimizeHpLoss => Ok(()),
        }
    }
}

pub trait CombatPolicy {
    fn kind(&self) -> PolicyKind;
    fn hp_loss(&self, combat_start_hp: i32, current_hp: i32) -> i32;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MinimizeHpLoss;

impl CombatPolicy for MinimizeHpLoss {
    fn kind(&self) -> PolicyKind {
        PolicyKind::MinimizeHpLoss
    }

    fn hp_loss(&self, combat_start_hp: i32, current_hp: i32) -> i32 {
        combat_start_hp - current_hp
    }
}
