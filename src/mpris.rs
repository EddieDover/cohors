use anyhow::Result;
use mpris_server::{
    LoopStatus, PlaybackStatus, PlayerInterface, Property, RootInterface, Server, Time, Volume,
};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedReceiver;
use zbus::fdo;

#[derive(Debug, Clone)]
pub struct MprisState {
    pub playback_status: PlaybackStatus,
    pub loop_status: LoopStatus,
    pub volume: f64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<std::time::Duration>,
    pub position: std::time::Duration,
    pub playback_start: Option<std::time::Instant>,
}

impl Default for MprisState {
    fn default() -> Self {
        Self {
            playback_status: PlaybackStatus::Stopped,
            loop_status: LoopStatus::None,
            volume: 1.0,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            duration: None,
            position: std::time::Duration::from_secs(0),
            playback_start: None,
        }
    }
}

#[derive(Debug)]
pub enum MprisCommand {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    Volume(f64),
    LoopStatus(LoopStatus),
    #[allow(dead_code)]
    Seek(i64), // Offset in microseconds
    #[allow(dead_code)]
    SetPosition(String, i64), // TrackId, Position in microseconds
}

pub struct CohorsMpris {
    tx: Sender<MprisCommand>,
    state: Arc<Mutex<MprisState>>,
}

impl CohorsMpris {
    pub fn new(tx: Sender<MprisCommand>, state: Arc<Mutex<MprisState>>) -> Self {
        Self { tx, state }
    }
}

impl RootInterface for CohorsMpris {
    async fn identity(&self) -> fdo::Result<String> {
        Ok("Cohors".to_string())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("cohors".to_string())
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![
            "audio/mpeg".to_string(),
            "audio/flac".to_string(),
            "audio/ogg".to_string(),
        ])
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec![
            "file".to_string(),
            "http".to_string(),
            "https".to_string(),
        ])
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn quit(&self) -> fdo::Result<()> {
        Ok(())
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn raise(&self) -> fdo::Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> zbus::Result<()> {
        Ok(())
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }
}

impl PlayerInterface for CohorsMpris {
    async fn next(&self) -> fdo::Result<()> {
        self.tx.send(MprisCommand::Next).ok();
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        self.tx.send(MprisCommand::Previous).ok();
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.tx.send(MprisCommand::Pause).ok();
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.tx.send(MprisCommand::PlayPause).ok();
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.tx.send(MprisCommand::Stop).ok();
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        self.tx.send(MprisCommand::Play).ok();
        Ok(())
    }

    async fn seek(&self, offset: Time) -> fdo::Result<()> {
        self.tx.send(MprisCommand::Seek(offset.as_micros())).ok();
        Ok(())
    }

    async fn set_position(
        &self,
        track_id: mpris_server::TrackId,
        position: Time,
    ) -> fdo::Result<()> {
        self.tx
            .send(MprisCommand::SetPosition(
                track_id.as_str().to_string(),
                position.as_micros(),
            ))
            .ok();
        Ok(())
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Ok(())
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        let state = self.state.lock().unwrap();
        Ok(state.volume)
    }

    async fn set_volume(&self, volume: Volume) -> zbus::Result<()> {
        self.tx.send(MprisCommand::Volume(volume)).ok();
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        let state = self.state.lock().unwrap();
        let mut pos = state.position;
        if let Some(start) = state.playback_start
            && state.playback_status == PlaybackStatus::Playing
        {
            pos += start.elapsed();
        }
        Ok(Time::from_micros(pos.as_micros() as i64))
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        let state = self.state.lock().unwrap();
        Ok(state.playback_status)
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        let state = self.state.lock().unwrap();
        Ok(state.loop_status)
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> zbus::Result<()> {
        self.tx.send(MprisCommand::LoopStatus(loop_status)).ok();
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: f64) -> zbus::Result<()> {
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> zbus::Result<()> {
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<mpris_server::Metadata> {
        let state = self.state.lock().unwrap();
        let mut metadata = mpris_server::Metadata::new();

        metadata.set(
            "mpris:trackid",
            "/org/mpris/MediaPlayer2/TrackList/NoTrack".into(),
        );

        if !state.title.is_empty() {
            metadata.set("xesam:title", state.title.clone().into());
        }
        if !state.artist.is_empty() {
            metadata.set("xesam:artist", vec![state.artist.clone()].into());
        }
        if !state.album.is_empty() {
            metadata.set("xesam:album", state.album.clone().into());
        }
        if let Some(duration) = state.duration {
            metadata.set("mpris:length", (duration.as_micros() as i64).into());
        }

        Ok(metadata)
    }

    async fn minimum_rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

pub struct MprisHandler {
    #[allow(dead_code)]
    pub server: Arc<Server<CohorsMpris>>,
}

impl MprisHandler {
    pub async fn new(
        tx: Sender<MprisCommand>,
        state: Arc<Mutex<MprisState>>,
        mut signal_rx: UnboundedReceiver<()>,
    ) -> Result<Self> {
        let imp = CohorsMpris::new(tx, state.clone());
        let server = Server::new("cohors", imp).await?;
        let server = Arc::new(server);

        let server_clone = server.clone();
        let state_clone = state.clone();

        tokio::spawn(async move {
            while signal_rx.recv().await.is_some() {
                let (playback_status, loop_status, volume, metadata) = {
                    let state = state_clone.lock().unwrap();

                    let mut metadata = mpris_server::Metadata::new();
                    metadata.set(
                        "mpris:trackid",
                        "/org/mpris/MediaPlayer2/TrackList/NoTrack".into(),
                    );

                    if !state.title.is_empty() {
                        metadata.set("xesam:title", state.title.clone().into());
                    }
                    if !state.artist.is_empty() {
                        metadata.set("xesam:artist", vec![state.artist.clone()].into());
                    }
                    if !state.album.is_empty() {
                        metadata.set("xesam:album", state.album.clone().into());
                    }
                    if let Some(duration) = state.duration {
                        metadata.set("mpris:length", (duration.as_micros() as i64).into());
                    }

                    (
                        state.playback_status,
                        state.loop_status,
                        state.volume,
                        metadata,
                    )
                };

                server_clone
                    .properties_changed(vec![
                        Property::PlaybackStatus(playback_status),
                        Property::LoopStatus(loop_status),
                        Property::Volume(volume),
                        Property::Metadata(metadata),
                    ])
                    .await
                    .ok();
            }
        });

        Ok(Self { server })
    }
}

#[cfg(test)]
mod tests;
