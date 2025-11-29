use super::*;
use std::sync::mpsc;

#[tokio::test]
async fn test_mpris_methods() {
    let (tx, rx) = mpsc::channel();
    let state = Arc::new(Mutex::new(MprisState::default()));
    let mpris = CohorsMpris::new(tx, state.clone());

    mpris.play().await.unwrap();
    assert!(matches!(rx.try_recv(), Ok(MprisCommand::Play)));

    mpris.pause().await.unwrap();
    assert!(matches!(rx.try_recv(), Ok(MprisCommand::Pause)));

    mpris.play_pause().await.unwrap();
    assert!(matches!(rx.try_recv(), Ok(MprisCommand::PlayPause)));

    mpris.stop().await.unwrap();
    assert!(matches!(rx.try_recv(), Ok(MprisCommand::Stop)));

    mpris.next().await.unwrap();
    assert!(matches!(rx.try_recv(), Ok(MprisCommand::Next)));

    mpris.previous().await.unwrap();
    assert!(matches!(rx.try_recv(), Ok(MprisCommand::Previous)));

    mpris.set_volume(0.5).await.unwrap();
    if let Ok(MprisCommand::Volume(v)) = rx.try_recv() {
        assert_eq!(v, 0.5);
    } else {
        panic!("Expected Volume command");
    }

    // Test RootInterface methods
    assert_eq!(mpris.identity().await.unwrap(), "Cohors");
    assert_eq!(mpris.desktop_entry().await.unwrap(), "cohors");
    assert!(
        mpris
            .supported_mime_types()
            .await
            .unwrap()
            .contains(&"audio/mpeg".to_string())
    );
    assert!(
        mpris
            .supported_uri_schemes()
            .await
            .unwrap()
            .contains(&"file".to_string())
    );
    assert!(mpris.can_quit().await.unwrap());
    mpris.quit().await.unwrap();

    assert!(!mpris.can_raise().await.unwrap());
    mpris.raise().await.unwrap();

    assert!(!mpris.can_set_fullscreen().await.unwrap());
    assert!(!mpris.fullscreen().await.unwrap());
    mpris.set_fullscreen(true).await.unwrap();

    assert!(!mpris.has_track_list().await.unwrap());

    // Test PlayerInterface other methods
    mpris.seek(Time::from_micros(1000)).await.unwrap();
    if let Ok(MprisCommand::Seek(t)) = rx.try_recv() {
        assert_eq!(t, 1000);
    } else {
        panic!("Expected Seek command");
    }

    mpris
        .set_position(
            mpris_server::TrackId::try_from("/org/mpris/MediaPlayer2/TrackList/NoTrack").unwrap(),
            Time::from_micros(5000),
        )
        .await
        .unwrap();
    if let Ok(MprisCommand::SetPosition(_id, pos)) = rx.try_recv() {
        assert_eq!(pos, 5000);
    } else {
        panic!("Expected SetPosition command");
    }

    mpris
        .open_uri("file:///test.mp3".to_string())
        .await
        .unwrap();

    // Test Metadata
    {
        let mut state_guard = state.lock().unwrap();
        state_guard.title = "Test Title".to_string();
        state_guard.artist = "Test Artist".to_string();
        state_guard.album = "Test Album".to_string();
        state_guard.duration = Some(std::time::Duration::from_secs(60));
    }
    let _metadata = mpris.metadata().await.unwrap();

    let vol: f64 = mpris.volume().await.unwrap();
    assert_eq!(vol, 1.0);
    assert_eq!(mpris.position().await.unwrap().as_micros(), 0);
    assert_eq!(
        mpris.playback_status().await.unwrap(),
        PlaybackStatus::Stopped
    );
    assert_eq!(mpris.loop_status().await.unwrap(), LoopStatus::None);
    mpris.set_loop_status(LoopStatus::Track).await.unwrap();
    if let Ok(MprisCommand::LoopStatus(s)) = rx.try_recv() {
        assert_eq!(s, LoopStatus::Track);
    } else {
        panic!("Expected LoopStatus command");
    }

    assert_eq!(mpris.rate().await.unwrap(), 1.0);
    mpris.set_rate(1.5).await.unwrap();

    assert!(!mpris.shuffle().await.unwrap());
    mpris.set_shuffle(true).await.unwrap();

    assert_eq!(mpris.minimum_rate().await.unwrap(), 1.0);
    assert_eq!(mpris.maximum_rate().await.unwrap(), 1.0);

    assert!(mpris.can_go_next().await.unwrap());
    assert!(mpris.can_go_previous().await.unwrap());
    assert!(mpris.can_play().await.unwrap());
    assert!(mpris.can_pause().await.unwrap());
    assert!(mpris.can_seek().await.unwrap());
    assert!(mpris.can_control().await.unwrap());

    // Test Position calculation
    {
        let mut state_guard = state.lock().unwrap();
        state_guard.playback_status = PlaybackStatus::Playing;
        state_guard.position = std::time::Duration::from_secs(10);
        state_guard.playback_start =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(5));
    }
    let pos = mpris.position().await.unwrap().as_micros();
    assert!(pos >= 15_000_000);
}
