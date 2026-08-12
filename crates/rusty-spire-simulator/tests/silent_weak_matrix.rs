use rusty_spire_combat::{CombatSetupV1, initialize};
use rusty_spire_data::CombatCatalog;
use rusty_spire_simulator::{SolveLimits, solve};
use serde_json::{Value, json};

fn catalog() -> CombatCatalog {
    CombatCatalog::from_json(include_bytes!("../../../catalogs/combat_v0.107.1.json")).unwrap()
}

fn setup_value(catalog: &CombatCatalog, enemy: &str, seed: u64, hp: i32) -> Value {
    json!({
        "schema_version": 1,
        "catalog_sha256": catalog.sha256,
        "ascension_level": 0,
        "rng": {
            "run_seed": seed.to_string(),
            "profile": "isolated_combat_xoshiro_v1"
        },
        "character": {
            "id": "CHARACTER.SILENT",
            "current_hp": 70,
            "max_hp": 70
        },
        "deck": [
            {"id": "CARD.STRIKE_SILENT", "quantity": 5, "upgrade_level": 0},
            {"id": "CARD.DEFEND_SILENT", "quantity": 5, "upgrade_level": 0},
            {"id": "CARD.NEUTRALIZE", "quantity": 1, "upgrade_level": 0},
            {"id": "CARD.SURVIVOR", "quantity": 1, "upgrade_level": 0}
        ],
        "relics": [{"id": "RELIC.RING_OF_THE_SNAKE"}],
        "potions": [],
        "encounter": {
            "type": "custom",
            "enemies": [{"id": enemy, "current_hp": hp, "max_hp": hp}]
        }
    })
}

#[test]
fn converted_silent_weak_seed_matrix_keeps_equivalent_optima() {
    let evidence: Value = serde_json::from_str(include_str!(
        "../../../fixtures/evidence/silent_weak_seed_matrix.json"
    ))
    .unwrap();
    let catalog = catalog();
    for case in evidence["cases"].as_array().unwrap() {
        let (enemy, hp) = match case["enemy"].as_str().unwrap() {
            "nibbit" => ("MONSTER.NIBBIT", 44),
            "fuzzy_wurm_crawler" => ("MONSTER.FUZZY_WURM_CRAWLER", 56),
            "shrinker_beetle" => ("MONSTER.SHRINKER_BEETLE", 39),
            other => panic!("unknown evidence enemy {other}"),
        };
        let setup: CombatSetupV1 = serde_json::from_value(setup_value(
            &catalog,
            enemy,
            case["shuffle_seed"].as_u64().unwrap(),
            hp,
        ))
        .unwrap();
        let combat = initialize(&catalog, &setup, false).unwrap();
        let opening = combat
            .state
            .hand
            .iter()
            .map(|card| Value::String(card.model_id.clone()))
            .collect::<Vec<_>>();
        assert_eq!(opening, case["opening_hand"].as_array().unwrap().clone());
        let result = solve(
            &catalog,
            &combat,
            SolveLimits {
                max_states: 100_000,
                max_turns: 50,
                timeout_seconds: 5.0,
            },
        )
        .unwrap();
        assert!(
            result.won,
            "failed case {enemy} seed {}",
            setup.rng.run_seed
        );
        assert!(result.optimality_proven);
        assert_eq!(result.hp_loss, case["hp_loss"].as_i64().map(|v| v as i32));
        assert_eq!(result.final_hp, case["final_hp"].as_i64().map(|v| v as i32));
    }
}
