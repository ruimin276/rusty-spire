use serde::Serialize;
use serde_json::{Map, Value};
use thiserror::Error;

use crate::{
    CardInstance, CombatState, CombatStatus, Decision, EnemyState, Metrics, PlayerState,
    Provenance, RngBankState,
};

#[derive(Debug, Error)]
pub enum CanonicalError {
    #[error("cannot serialize canonical state: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Serialize a value with recursively sorted object keys and preserved arrays.
pub fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, CanonicalError> {
    let mut value = serde_json::to_value(value)?;
    canonicalize(&mut value);
    Ok(serde_json::to_vec(&value)?)
}

/// Stable BLAKE3 identity over the canonical snapshot representation.
pub fn state_id<T: Serialize>(value: &T) -> Result<String, CanonicalError> {
    Ok(blake3::hash(&canonical_json(value)?).to_hex().to_string())
}

/// Hot-path canonical combat identity. This dedicated DTO freezes the wire
/// field order independently of `CombatState`'s internal declaration layout
/// without constructing and sorting a generic JSON tree for every search node.
pub fn combat_state_id(state: &CombatState) -> Result<String, CanonicalError> {
    #[derive(Serialize)]
    struct CanonicalCombatState<'a> {
        snapshot_schema: u32,
        provenance: &'a Provenance,
        rng: &'a RngBankState,
        combat: &'a CombatStatus,
        decision: &'a Decision,
        player: &'a PlayerState,
        enemies: &'a [EnemyState],
        hand: &'a [CardInstance],
        draw_pile: &'a [CardInstance],
        discard_pile: &'a [CardInstance],
        exhaust_pile: &'a [CardInstance],
        play_pile: &'a [CardInstance],
        metrics: &'a Metrics,
    }

    let canonical = CanonicalCombatState {
        snapshot_schema: state.snapshot_schema,
        provenance: &state.provenance,
        rng: &state.rng,
        combat: &state.combat,
        decision: &state.decision,
        player: &state.player,
        enemies: &state.enemies,
        hand: &state.hand,
        draw_pile: &state.draw_pile,
        discard_pile: &state.discard_pile,
        exhaust_pile: &state.exhaust_pile,
        play_pile: &state.play_pile,
        metrics: &state.metrics,
    };
    Ok(blake3::hash(&serde_json::to_vec(&canonical)?)
        .to_hex()
        .to_string())
}

fn canonicalize(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(canonicalize),
        Value::Object(values) => {
            let old = std::mem::take(values);
            let mut entries = old.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = Map::new();
            for (key, mut value) in entries {
                canonicalize(&mut value);
                sorted.insert(key, value);
            }
            *values = sorted;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_declaration_order_does_not_change_state_id() {
        let first = serde_json::json!({"z": 1, "a": {"d": 2, "b": 3}, "items": [2, 1]});
        let second = serde_json::json!({"items": [2, 1], "a": {"b": 3, "d": 2}, "z": 1});
        assert_eq!(state_id(&first).unwrap(), state_id(&second).unwrap());
        assert_ne!(
            state_id(&first).unwrap(),
            state_id(&serde_json::json!({"items": [1, 2], "a": {"b": 3, "d": 2}, "z": 1})).unwrap()
        );
    }
}
