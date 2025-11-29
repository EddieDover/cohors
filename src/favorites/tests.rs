use super::*;
use std::env;
use std::sync::Mutex;
use tempfile::tempdir;

static ENV_MUTEX: Mutex<()> = Mutex::new(());

// Helper to run test with modified environment
fn with_xdg_config_home<F>(path: &std::path::Path, f: F)
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

#[test]
fn test_favorites_persistence() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().to_path_buf();

    with_xdg_config_home(&config_path, || {
        let mut favs = Favorites::default();
        favs.files.push(PathBuf::from("test_file.mp3"));
        let station = RadioStation {
            name: "Test".to_string(),
            url: "http://test.com".to_string(),
            description: None,
            homepage: None,
            tags: None,
            last_playing: None,
        };
        favs.stations.push(station.clone());

        // Test Save
        assert!(favs.save().is_ok());

        // Verify file exists
        let expected_path = config_path.join("cohors").join("favorites.json");
        assert!(expected_path.exists());

        // Test Load
        let loaded = Favorites::load();
        assert_eq!(loaded.files.len(), 1);
        assert_eq!(loaded.stations.len(), 1);
        assert_eq!(loaded.files[0], PathBuf::from("test_file.mp3"));
        assert_eq!(loaded.stations[0].name, "Test");
    });
}

#[test]
fn test_favorites_load_empty() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().to_path_buf();

    with_xdg_config_home(&config_path, || {
        let loaded = Favorites::load();
        assert!(loaded.files.is_empty());
        assert!(loaded.stations.is_empty());
    });
}

#[test]
fn test_favorites_load_corrupted() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().to_path_buf();

    with_xdg_config_home(&config_path, || {
        let cohors_dir = config_path.join("cohors");
        fs::create_dir_all(&cohors_dir).unwrap();
        let favorites_path = cohors_dir.join("favorites.json");
        fs::write(&favorites_path, "{ invalid json").unwrap();

        let loaded = Favorites::load();
        assert!(loaded.files.is_empty());
        assert!(loaded.stations.is_empty());

        // Check backup
        let backup_path = favorites_path.with_extension("json.bak");
        assert!(backup_path.exists());
        let content = fs::read_to_string(backup_path).unwrap();
        assert_eq!(content, "{ invalid json");
    });
}
