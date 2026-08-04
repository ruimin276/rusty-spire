use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

#[pyfunction]
#[pyo3(signature = (scenario_json, limits_json=None))]
fn solve_simulator(scenario_json: &str, limits_json: Option<&str>) -> PyResult<String> {
    sls2_combat_core::solve_scenario_json(scenario_json, limits_json)
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

#[pyfunction]
#[pyo3(signature = (baseline_json, candidate_json, limits_json=None))]
fn compare_simulators(
    baseline_json: &str,
    candidate_json: &str,
    limits_json: Option<&str>,
) -> PyResult<String> {
    sls2_combat_core::compare_scenarios_json(baseline_json, candidate_json, limits_json)
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

#[pyfunction]
fn validate_snapshot(scenario_json: &str) -> PyResult<String> {
    sls2_combat_core::validate_scenario_json(scenario_json)
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

#[pyfunction]
fn step_simulator(scenario_json: &str, action_json: &str) -> PyResult<String> {
    sls2_combat_core::step_scenario_json(scenario_json, action_json)
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

#[pyfunction]
fn prepare_simulator(scenario_json: &str) -> PyResult<String> {
    sls2_combat_core::prepare_scenario_json(scenario_json)
        .map_err(|error| PyValueError::new_err(error.to_string()))
}

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(solve_simulator, module)?)?;
    module.add_function(wrap_pyfunction!(compare_simulators, module)?)?;
    module.add_function(wrap_pyfunction!(validate_snapshot, module)?)?;
    module.add_function(wrap_pyfunction!(step_simulator, module)?)?;
    module.add_function(wrap_pyfunction!(prepare_simulator, module)?)?;
    Ok(())
}
