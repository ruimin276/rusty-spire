use rusty_spire_combat::LegacyPolicyKind;

pub type ObjectiveKind = LegacyPolicyKind;

pub trait CombatObjective {
    fn kind(&self) -> ObjectiveKind;
    fn path_cost(&self, combat_start_hp: i32, current_hp: i32) -> i32;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MinimizeHpLoss;

impl CombatObjective for MinimizeHpLoss {
    fn kind(&self) -> ObjectiveKind {
        ObjectiveKind::MinimizeHpLoss
    }

    fn path_cost(&self, combat_start_hp: i32, current_hp: i32) -> i32 {
        combat_start_hp - current_hp
    }
}
