#[cfg(target_arch = "wasm32")]
mod browser {
    use std::slice;

    use serde_json::{Value, json};
    use sls2_combat_core::{CombatCatalog, CombatSetupV1, SolveLimits, initialize, solve};

    const CATALOG_JSON: &[u8] = include_bytes!("../../../catalogs/combat_v0.107.1.json");

    #[unsafe(no_mangle)]
    pub extern "C" fn sls2_alloc(length: u32) -> u32 {
        let buffer = vec![0_u8; length as usize].into_boxed_slice();
        Box::into_raw(buffer) as *mut u8 as u32
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn sls2_free(pointer: u32, length: u32) {
        if pointer == 0 {
            return;
        }
        let slice = std::ptr::slice_from_raw_parts_mut(pointer as *mut u8, length as usize);
        unsafe {
            drop(Box::from_raw(slice));
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn sls2_solve_json(
        pointer: u32,
        length: u32,
        max_states: u32,
        max_turns: u32,
        timeout_millis: u32,
    ) -> u64 {
        let response = if pointer == 0 {
            Err("input pointer cannot be zero".to_owned())
        } else {
            let input = unsafe { slice::from_raw_parts(pointer as *const u8, length as usize) };
            solve_input(
                input,
                SolveLimits {
                    max_states: max_states as usize,
                    max_turns,
                    timeout_seconds: f64::from(timeout_millis) / 1_000.0,
                },
            )
        };
        let envelope = match response {
            Ok(value) => json!({ "ok": true, "value": value }),
            Err(error) => json!({ "ok": false, "error": error }),
        };
        leak_bytes(serde_json::to_vec(&envelope).expect("JSON envelope is serializable"))
    }

    fn solve_input(input: &[u8], limits: SolveLimits) -> Result<Value, String> {
        let catalog = CombatCatalog::from_json(CATALOG_JSON).map_err(|error| error.to_string())?;
        let setup: CombatSetupV1 = serde_json::from_slice(input)
            .map_err(|error| format!("invalid CombatSetupV1: {error}"))?;
        let combat = initialize(&catalog, &setup, false).map_err(|error| error.to_string())?;
        let opening_hand = combat
            .state
            .hand
            .iter()
            .map(|card| card.model_id.clone())
            .collect::<Vec<_>>();
        let result = solve(&catalog, &combat, limits).map_err(|error| error.to_string())?;
        Ok(json!({ "result": result, "opening_hand": opening_hand }))
    }

    fn leak_bytes(bytes: Vec<u8>) -> u64 {
        let bytes = bytes.into_boxed_slice();
        let length = bytes.len() as u32;
        let pointer = Box::into_raw(bytes) as *mut u8 as u32;
        (u64::from(length) << 32) | u64::from(pointer)
    }
}
