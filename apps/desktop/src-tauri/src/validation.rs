// Compiled only for the native validation harness; never enable in release artifacts.
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};
use tauri::{Listener, Manager};

static STARTED: OnceLock<Instant> = OnceLock::new();
static STAGES: Mutex<Vec<serde_json::Value>> = Mutex::new(Vec::new());
static PROFILING: AtomicBool = AtomicBool::new(true);
static PLAN: OnceLock<Option<serde_json::Value>> = OnceLock::new();
static TIMING_DROPPED: AtomicUsize = AtomicUsize::new(0);

fn response_diagnostics() -> bool {
    PLAN.get()
        .and_then(Option::as_ref)
        .is_some_and(|plan| plan["responseDiagnostics"] == true)
}

fn append_stage(stages: &mut Vec<serde_json::Value>, stage: serde_json::Value) -> bool {
    if stages.len() >= 1024 {
        return false;
    }
    stages.push(stage);
    true
}

pub fn plan() -> Option<serde_json::Value> {
    PLAN.get().cloned().flatten()
}

pub struct Stage {
    name: &'static str,
    started: Instant,
    items: usize,
}

impl Stage {
    pub fn start(name: &'static str, items: usize) -> Self {
        Self {
            name,
            started: Instant::now(),
            items,
        }
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        // Unit tests run without app setup; they must not retain timings.
        if let Some(origin) = STARTED.get() {
            if !PROFILING.load(Ordering::Relaxed) && !response_diagnostics() {
                return;
            }
            if !append_stage(
                &mut STAGES.lock().unwrap(),
                serde_json::json!({
                    "name": self.name,
                    "startMs": self.started.duration_since(*origin).as_secs_f64() * 1000.0,
                    "durationMs": self.started.elapsed().as_secs_f64() * 1000.0,
                    "items": self.items,
                }),
            ) {
                TIMING_DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

pub fn directory() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Fail closed instead of ever opening a user's normal profile.
    Ok(PathBuf::from(
        std::env::var_os("NIMBLEPOST_VALIDATION_DIR")
            .ok_or("native-validation requires NIMBLEPOST_VALIDATION_DIR")?,
    )
    .canonicalize()?)
}

pub fn setup(
    app: &tauri::App,
    started: Instant,
    directory: PathBuf,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let profile = directory.join("profile");
    fs::create_dir_all(&profile)?;
    let _ = STARTED.set(started);
    let plan_path = directory.join("plan.json");
    let _ = PLAN.set(if plan_path.exists() {
        Some(serde_json::from_slice(&fs::read(plan_path)?)?)
    } else {
        None
    });
    for stage in ["ready", "profile", "memory", "responses", "smoke"] {
        let directory = directory.clone();
        let handle = app.handle().clone();
        app.listen(format!("native-validation-{stage}"), move |event| {
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                let payload: serde_json::Value = serde_json::from_str(event.payload())?;
                let filename = if stage == "memory" || stage == "responses" {
                    let index = payload["index"]
                        .as_u64()
                        .filter(|index| *index < if stage == "responses" { 64 } else { 16 })
                        .ok_or("Invalid memory phase")?;
                    format!("{stage}-{index}.json")
                } else {
                    format!("{stage}.json")
                };
                let backend_stages = if response_diagnostics() {
                    std::mem::take(&mut *STAGES.lock().unwrap())
                } else {
                    STAGES.lock().unwrap().clone()
                };
                let report = serde_json::json!({
                    "buildSha": option_env!("NIMBLEPOST_BUILD_SHA"),
                    "version": env!("CARGO_PKG_VERSION"),
                    "debugAssertions": cfg!(debug_assertions),
                    "inspector": cfg!(feature = "native-inspector"),
                    "backendStartupMs": started.elapsed().as_secs_f64() * 1000.0,
                    "payload": payload,
                    "backendStages": backend_stages,
                    "backendTimingDropped": TIMING_DROPPED.swap(0, Ordering::Relaxed),
                    "backendMemory": handle.state::<crate::session::Session>().validation_memory(),
                });
                let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
                temporary.write_all(&serde_json::to_vec(&report)?)?;
                temporary.persist(directory.join(filename))?;
                #[cfg(feature = "native-inspector")]
                if stage == "ready" {
                    if let Some(window) = handle.get_webview_window("main") {
                        window.open_devtools();
                    }
                }
                if stage == "profile" && plan().is_some() {
                    // Sustained usage must not measure growing diagnostic buffers.
                    PROFILING.store(false, Ordering::Relaxed);
                    STAGES.lock().unwrap().clear();
                }
                Ok(())
            })();
            if let Err(error) = result {
                eprintln!("Native validation report failed: {error}");
            }
        });
    }
    Ok(profile)
}

#[cfg(test)]
mod tests {
    #[test]
    fn diagnostic_timings_cannot_grow_without_bound() {
        let mut stages = Vec::new();
        for _ in 0..1024 {
            assert!(super::append_stage(&mut stages, serde_json::Value::Null));
        }
        assert!(!super::append_stage(&mut stages, serde_json::Value::Null));
        assert_eq!(stages.len(), 1024);
        stages.clear();
        assert!(super::append_stage(&mut stages, serde_json::Value::Null));
    }
}
