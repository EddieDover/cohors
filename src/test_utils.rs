use std::env;
use std::sync::Mutex;

pub static ENV_MUTEX: Mutex<()> = Mutex::new(());

// Helper to run test with modified environment
pub fn with_xdg_config_home<F>(path: &std::path::Path, f: F)
where
    F: FnOnce(),
{
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let key = "XDG_CONFIG_HOME";
    let old_val = env::var_os(key);
    unsafe {
        env::set_var(key, path);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    unsafe {
        if let Some(val) = old_val {
            env::set_var(key, val);
        } else {
            env::remove_var(key);
        }
    }
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
