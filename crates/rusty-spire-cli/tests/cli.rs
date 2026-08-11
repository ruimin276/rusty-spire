use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rusty-spire"))
}

fn fixture(name: &str) -> PathBuf {
    root().join("fixtures/combat_setup_v1").join(name)
}

fn catalog() -> PathBuf {
    root().join("catalogs/combat_v0.107.1.json")
}

fn run_json(arguments: &[&str]) -> Value {
    let output = binary().args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn temporary_json(name: &str, value: &Value) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("rusty-spire-{name}-{unique}.json"));
    fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}

#[test]
fn validate_and_catalog_info_report_identity() {
    let catalog_path = catalog();
    let input = fixture("silent_nibbit_seed_1.json");
    let validated = run_json(&[
        "validate",
        "--catalog",
        catalog_path.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
    ]);
    assert_eq!(validated["ok"], true);
    assert_eq!(validated["policy"], "minimize_hp_loss");

    let info = run_json(&["catalog-info", "--catalog", catalog_path.to_str().unwrap()]);
    assert_eq!(info["schema_version"], 1);
    assert_eq!(info["counts"]["monsters"], 3);
}

#[test]
fn solve_supports_default_explicit_and_incomplete_searches() {
    let catalog_path = catalog();
    let input = fixture("silent_nibbit_seed_1.json");
    let common = [
        "--catalog",
        catalog_path.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
    ];

    let mut args = vec!["solve"];
    args.extend(common);
    let default = run_json(&args);
    assert_eq!(default["won"], true);
    assert_eq!(default["hp_loss"], 0);
    assert_eq!(default["optimality_proven"], true);

    let mut args = vec!["solve"];
    args.extend(common);
    args.extend(["--policy", "minimize-hp-loss"]);
    let explicit = run_json(&args);
    assert_eq!(explicit["action_ids"], default["action_ids"]);

    let mut args = vec!["solve"];
    args.extend(common);
    args.extend(["--max-states", "0"]);
    let incomplete = run_json(&args);
    assert_eq!(incomplete["complete"], false);
    assert_eq!(incomplete["optimality_proven"], false);
    assert_eq!(incomplete["termination_reason"], "max_states");

    let mut args = vec!["solve"];
    args.extend(common);
    args.extend(["--max-turns", "0"]);
    let incomplete = run_json(&args);
    assert_eq!(incomplete["optimality_proven"], false);
    assert_eq!(incomplete["termination_reason"], "max_turns");
}

#[test]
fn compare_reports_hp_loss_delta() {
    let catalog_path = catalog();
    let baseline = fixture("silent_nibbit_seed_1.json");
    let candidate = fixture("silent_fuzzy_seed_4.json");
    let result = run_json(&[
        "compare",
        "--catalog",
        catalog_path.to_str().unwrap(),
        "--baseline",
        baseline.to_str().unwrap(),
        "--candidate",
        candidate.to_str().unwrap(),
    ]);
    assert_eq!(result["hp_loss_delta"], 1);
    assert_eq!(result["better"], "baseline");
}

#[test]
fn converted_ironclad_nibbit_golden_keeps_the_verified_outcome() {
    let catalog_path = catalog();
    let input = fixture("ironclad_nibbit_seed_1.json");
    let result = run_json(&[
        "solve",
        "--catalog",
        catalog_path.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
    ]);
    assert_eq!(result["won"], true);
    assert_eq!(result["complete"], true);
    assert_eq!(result["optimality_proven"], true);
    assert_eq!(result["hp_loss"], 2);
    assert_eq!(result["final_hp"], 78);
    assert_eq!(result["actions"].as_array().unwrap().len(), 14);
}

#[test]
fn malformed_and_multi_enemy_inputs_fail_closed() {
    let catalog_path = catalog();
    let input = fixture("silent_nibbit_seed_1.json");
    let mut setup: Value = serde_json::from_slice(&fs::read(input).unwrap()).unwrap();
    setup["unknown_field"] = true.into();
    let malformed = temporary_json("malformed", &setup);
    let output = binary()
        .args([
            "validate",
            "--catalog",
            catalog_path.to_str().unwrap(),
            "--input",
            malformed.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());

    setup.as_object_mut().unwrap().remove("unknown_field");
    setup["encounter"] = serde_json::json!({
        "type": "catalog",
        "id": "ENCOUNTER.NIBBITS_NORMAL"
    });
    let multi = temporary_json("multi", &setup);
    let output = binary()
        .args([
            "validate",
            "--catalog",
            catalog_path.to_str().unwrap(),
            "--input",
            multi.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("multi-enemy"));

    let _ = fs::remove_file(malformed);
    let _ = fs::remove_file(multi);
}
