//! Optional search ordering heuristics. These never own combat rules.

use rusty_spire_core::CombatState;
use rusty_spire_simulator::Heuristic;

/// Orders states with less remaining enemy HP first. It is only an ordering
/// signal and therefore cannot invalidate exact-search optimality.
#[derive(Clone, Copy, Debug, Default)]
pub struct RemainingEnemyHp;

impl Heuristic for RemainingEnemyHp {
    fn estimate(&self, state: &CombatState) -> i32 {
        state.enemies.iter().map(|enemy| enemy.hp.max(0)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_state_has_zero_remaining_hp() {
        let state: CombatState = serde_json::from_value(serde_json::json!({
            "snapshot_schema": 2,
            "provenance": {"game_version": "test"},
            "rng": {"algorithm": "test"},
            "combat": {"won": true, "turn": 1},
            "decision": {"kind": "terminal"},
            "player": {"combat_id": "p", "model_id": "CHARACTER.TEST", "hp": 1, "max_hp": 1, "energy": 0, "max_energy": 0},
            "enemies": []
        })).unwrap();
        assert_eq!(RemainingEnemyHp.estimate(&state), 0);
    }
}
