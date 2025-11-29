use super::*;
use crate::mpris::MprisCommand;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn test_mpris_command_handling() {
    let mut app = App::new_test();

    // Test Volume
    app.handle_mpris_command(MprisCommand::Volume(0.5));
    assert_eq!(app.volume, 0.5);

    // Test Loop Status
    app.handle_mpris_command(MprisCommand::LoopStatus(mpris_server::LoopStatus::Track));
    assert!(matches!(app.loop_mode, LoopMode::Track));

    app.handle_mpris_command(MprisCommand::LoopStatus(mpris_server::LoopStatus::Playlist));
    assert!(matches!(app.loop_mode, LoopMode::All));

    app.handle_mpris_command(MprisCommand::LoopStatus(mpris_server::LoopStatus::None));
    assert!(matches!(app.loop_mode, LoopMode::Off));

    // Test Play (when paused)
    app.is_paused = true;
    app.handle_mpris_command(MprisCommand::Play);
    assert!(!app.is_paused);

    // Test Seek (basic logic check, not actual sink seek)
    app.track_duration = Some(Duration::from_secs(100));
    app.playback_elapsed = Duration::from_secs(10);

    // We can't easily test the actual seek effect without a real sink/source,
    // but we can verify it doesn't panic or crash.
    // The seek_to method will try to use the sink, which is a mock/idle sink in tests.
    app.handle_mpris_command(MprisCommand::Seek(5000000)); // +5s
    // Since sink is idle/mock, it might not update playback_elapsed if try_seek fails or does nothing.
    // But we exercised the code path.
}

#[test]
fn test_update_mpris_state() {
    let mut app = App::new_test();
    let mpris_state =
        std::sync::Arc::new(std::sync::Mutex::new(crate::mpris::MprisState::default()));
    app.mpris_state = Some(mpris_state.clone());

    app.volume = 0.8;
    app.loop_mode = LoopMode::Track;
    app.current_track = Some(PathBuf::from("test_song.mp3"));
    app.track_duration = Some(Duration::from_secs(180));

    app.update_mpris();

    let state = mpris_state.lock().unwrap();
    // Use a larger epsilon for f32 to f64 conversion
    assert!((state.volume - 0.8).abs() < 1e-6);
    assert!(matches!(state.loop_status, mpris_server::LoopStatus::Track));
    assert_eq!(state.title, "test_song.mp3");
    assert_eq!(state.duration, Some(Duration::from_secs(180)));
}
