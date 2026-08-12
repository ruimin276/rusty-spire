#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

pub(crate) struct SearchTimer {
    #[cfg(not(target_arch = "wasm32"))]
    started: Instant,
    #[cfg(target_arch = "wasm32")]
    started_seconds: f64,
}

impl SearchTimer {
    pub(crate) fn start() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            started: Instant::now(),
            #[cfg(target_arch = "wasm32")]
            started_seconds: now_seconds(),
        }
    }

    pub(crate) fn elapsed_seconds(&self) -> f64 {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.started.elapsed().as_secs_f64()
        }
        #[cfg(target_arch = "wasm32")]
        {
            now_seconds() - self.started_seconds
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn sls2_now_ms() -> f64;
}

#[cfg(target_arch = "wasm32")]
fn now_seconds() -> f64 {
    // The static browser wrapper supplies performance.now() through this import.
    unsafe { sls2_now_ms() / 1_000.0 }
}
