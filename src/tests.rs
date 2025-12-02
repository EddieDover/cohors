use super::*;
use std::fs::File;
use tempfile::tempdir;

#[test]
fn test_apply_args_volume() {
    let mut app = App::new_test();
    let args = Args {
        volume: Some(50),
        radio: false,
        station_file: None,
        invalidate_cache: false,
        path: vec![],
    };
    apply_args(&mut app, args);
    assert_eq!(app.volume, 0.5);
}

#[test]
fn test_apply_args_radio() {
    let mut app = App::new_test();
    let args = Args {
        volume: None,
        radio: true,
        station_file: None,
        invalidate_cache: false,
        path: vec![],
    };
    apply_args(&mut app, args);
    match app.mode {
        app::AppMode::Radio => {}
        _ => panic!("App mode should be Radio"),
    }
}

#[test]
fn test_apply_args_file_path() {
    let mut app = App::new_test();
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.mp3");
    File::create(&file_path).unwrap();

    let args = Args {
        volume: None,
        radio: false,
        station_file: None,
        invalidate_cache: false,
        path: vec![file_path.to_string_lossy().to_string()],
    };
    apply_args(&mut app, args);

    assert_eq!(
        app.current_dir.canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );
    // play_file uses rodio which might fail or be async/threaded
    // Check if the item is selected
    assert!(
        app.items
            .iter()
            .any(|p| p.file_name() == file_path.file_name())
    );
}

#[test]
fn test_apply_args_file_path_with_spaces() {
    let mut app = App::new_test();
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test file.mp3");
    File::create(&file_path).unwrap();

    let full_path_str = file_path.to_string_lossy().to_string();
    // If the user passes /path/to/test file.mp3 without quotes,
    // shell gives: /path/to/test, file.mp3

    // Split the full path by space to simulate what clap receives
    let parts: Vec<String> = full_path_str.split(' ').map(|s| s.to_string()).collect();

    let args = Args {
        volume: None,
        radio: false,
        station_file: None,
        invalidate_cache: false,
        path: parts,
    };

    apply_args(&mut app, args);

    assert_eq!(
        app.current_dir.canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );
    assert!(
        app.items
            .iter()
            .any(|p| p.file_name() == file_path.file_name())
    );
}

#[test]
fn test_apply_args_dir_path() {
    let mut app = App::new_test();
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.mp3");
    File::create(&file_path).unwrap();

    let args = Args {
        volume: None,
        radio: false,
        station_file: None,
        invalidate_cache: false,
        path: vec![dir.path().to_string_lossy().to_string()],
    };
    apply_args(&mut app, args);

    assert_eq!(
        app.current_dir.canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );
    match app.loop_mode {
        app::LoopMode::All => {}
        _ => panic!("Loop mode should be All"),
    }
}
