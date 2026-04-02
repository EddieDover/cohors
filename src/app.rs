use crate::audio::AudioAnalyzer;
use crate::config::AppConfig;
use crate::favorites::Favorites;
use crate::mpris::MprisCommand;
use crate::radio::{RadioGroup, RadioStation};
use crate::ui;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::widgets::ListState;
use ratatui::{Terminal, backend::Backend};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use serde::Deserialize;
use std::{
    fs, io,
    path::PathBuf,
    sync::mpsc::Receiver,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub type AudioSource = Box<dyn Source<Item = f32> + Send>;
pub type SourceResult = Result<AudioSource, String>;
pub type SourceReceiver = std::sync::mpsc::Receiver<SourceResult>;

#[derive(Deserialize)]
#[allow(dead_code)]
struct GitHubRelease {
    tag_name: String,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AppMode {
    FileSystem,
    Radio,
    Favorites,
    Subsonic,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SubsonicView {
    Servers,
    Artists,
    Albums(String),
    Tracks(String),
}

pub enum LoopMode {
    Off,
    Track,
    All,
}

#[allow(clippy::enum_variant_names)]
pub enum ConfirmationContext {
    DeleteStation(String),
    DeleteSource(String),
    DeleteSubsonic(String),
}

pub enum AddModalState {
    Selection,
    InputStation {
        name: String,
        url: String,
        description: String,
        homepage: String,
        tags: String,
        focused_field: usize,
        original_url: Option<String>,
    },
    InputSource {
        title: String,
        json_url: String,
        container: String,
        map_name: String,
        map_url: String,
        map_desc: String,
        map_home: String,
        map_tags: String,
        focused_field: usize,
        original_title: Option<String>,
    },
    InputSubsonic {
        server_url: String,
        username: String,
        password: String,
        focused_field: usize,
        original_url: Option<String>,
    },
    Confirmation {
        message: String,
        context: ConfirmationContext,
    },
}

pub struct App {
    pub mode: AppMode,
    pub favorites: Favorites,
    pub current_dir: PathBuf,
    pub items: Vec<PathBuf>,
    pub state: ListState,
    // Radio
    pub radio_groups: Vec<RadioGroup>,
    pub radio_state: ListState,
    pub station_receiver: Option<std::sync::mpsc::Receiver<Result<Vec<RadioGroup>, String>>>,
    // Favorites
    pub favorites_state: ListState,
    // Audio
    pub _stream: Option<OutputStream>,
    pub _stream_handle: Option<OutputStreamHandle>,
    pub sink: Option<Sink>,
    pub volume: f32,
    pub current_track: Option<PathBuf>,
    pub is_paused: bool,
    pub last_error: Option<String>,
    // Playback state
    pub track_duration: Option<Duration>,
    pub playback_start: Option<Instant>,
    pub playback_elapsed: Duration,
    // Visualizer
    pub spectrum_data: Arc<Mutex<Vec<(&'static str, u64)>>>,
    // Async Source Loading
    pub source_receiver: Option<SourceReceiver>,
    // HTTP Client
    pub http_client: reqwest::blocking::Client,
    // UI State
    pub show_help: bool,
    pub show_hidden: bool,
    // Looping Mode
    pub loop_mode: LoopMode,
    // Search
    pub is_searching: bool,
    pub search_query: String,
    pub filtered_items: Vec<PathBuf>,
    pub filtered_radio_groups: Vec<RadioGroup>,
    pub mpris_state: Option<std::sync::Arc<std::sync::Mutex<crate::mpris::MprisState>>>,
    pub mpris_notifier: Option<tokio::sync::mpsc::UnboundedSender<()>>,
    // Version
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_receiver: Option<std::sync::mpsc::Receiver<Option<String>>>,
    pub notification: Option<(String, Instant)>,
    pub add_modal_state: Option<AddModalState>,
    // Subsonic
    pub subsonic_clients: Vec<crate::subsonic::SubsonicClient>,
    pub active_subsonic_client: usize,
    pub subsonic_state: ratatui::widgets::ListState,
    pub subsonic_artists: Vec<crate::subsonic::Artist>,
    pub subsonic_albums: Vec<crate::subsonic::Album>,
    pub subsonic_tracks: Vec<crate::subsonic::Track>,
    pub subsonic_view: SubsonicView,
}

impl App {
    pub fn new() -> App {
        let (stream, stream_handle, sink, error) = match OutputStream::try_default() {
            Ok((s, h)) => match Sink::try_new(&h) {
                Ok(sink) => (Some(s), Some(h), Some(sink), None),
                Err(e) => (Some(s), Some(h), None, Some(format!("Sink error: {}", e))),
            },
            Err(e) => (None, None, None, Some(format!("Audio init error: {}", e))),
        };

        let volume = 0.5;
        if let Some(s) = &sink {
            s.set_volume(volume);
        }

        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut app = App {
            mode: AppMode::FileSystem,
            favorites: Favorites::default(),
            current_dir,
            items: Vec::new(),
            state: ListState::default(),
            radio_groups: Vec::new(),
            radio_state: ListState::default(),
            station_receiver: None,
            favorites_state: ListState::default(),
            _stream: stream,
            _stream_handle: stream_handle,
            sink,
            volume,
            current_track: None,
            is_paused: false,
            last_error: error,
            track_duration: None,
            playback_start: None,
            playback_elapsed: Duration::from_secs(0),
            spectrum_data: Arc::new(Mutex::new(vec![
                ("Sub", 0),
                ("Bass", 0),
                ("LowM", 0),
                ("Mid", 0),
                ("HighM", 0),
                ("Pres", 0),
                ("Bril", 0),
                ("Air", 0),
            ])),
            source_receiver: None,
            http_client: reqwest::blocking::Client::new(),
            show_help: false,
            show_hidden: false,
            loop_mode: LoopMode::Off,
            is_searching: false,
            search_query: String::new(),
            filtered_items: Vec::new(),
            filtered_radio_groups: Vec::new(),
            mpris_state: None,
            mpris_notifier: None,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            latest_version: None,
            update_receiver: None,
            notification: None,
            add_modal_state: None,
            // Subsonic
            subsonic_clients: Vec::new(),
            active_subsonic_client: 0,
            subsonic_state: ListState::default(),
            subsonic_artists: Vec::new(),
            subsonic_albums: Vec::new(),
            subsonic_tracks: Vec::new(),
            subsonic_view: SubsonicView::Servers,
        };
        app.check_for_updates();
        app.load_directory();
        app
    }

    #[cfg(test)]
    pub fn new_test() -> App {
        let (sink, queue) = Sink::new_idle();
        std::mem::forget(queue);
        App {
            mode: AppMode::FileSystem,
            favorites: Favorites::default(),
            current_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            items: Vec::new(),
            state: ListState::default(),
            radio_groups: Vec::new(),
            radio_state: ListState::default(),
            station_receiver: None,
            favorites_state: ListState::default(),
            _stream: None,
            _stream_handle: None,
            sink: Some(sink),
            volume: 0.5,
            current_track: None,
            is_paused: false,
            last_error: None,
            track_duration: None,
            playback_start: None,
            playback_elapsed: Duration::from_secs(0),
            spectrum_data: Arc::new(Mutex::new(vec![
                ("Sub", 0),
                ("Bass", 0),
                ("LowM", 0),
                ("Mid", 0),
                ("HighM", 0),
                ("Pres", 0),
                ("Bril", 0),
                ("Air", 0),
            ])),
            source_receiver: None,
            http_client: reqwest::blocking::Client::new(),
            show_help: false,
            show_hidden: false,
            loop_mode: LoopMode::Off,
            is_searching: false,
            search_query: String::new(),
            filtered_items: Vec::new(),
            filtered_radio_groups: Vec::new(),
            mpris_state: None,
            mpris_notifier: None,
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            latest_version: None,
            update_receiver: None,
            notification: None,
            add_modal_state: None,
            subsonic_clients: Vec::new(),
            active_subsonic_client: 0,
            subsonic_state: ratatui::widgets::ListState::default(),
            subsonic_artists: Vec::new(),
            subsonic_albums: Vec::new(),
            subsonic_tracks: Vec::new(),
            subsonic_view: SubsonicView::Servers,
        }
    }
    pub fn check_for_updates(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.update_receiver = Some(rx);
        let current_version = self.current_version.clone();

        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::builder()
                .user_agent("cohors")
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new());

            let url = "https://api.github.com/repos/EddieDover/cohors/releases/latest";
            if let Ok(resp) = client.get(url).send()
                && resp.status().is_success()
                && let Ok(release) = resp.json::<GitHubRelease>()
            {
                let latest = release.tag_name.trim_start_matches('v').to_string();
                if latest != current_version {
                    let _ = tx.send(Some(latest));
                    return;
                }
            }
            let _ = tx.send(None);
        });
    }

    pub fn load_directory(&mut self) {
        self.items.clear();

        // Add parent directory option if not at root
        if self.current_dir.parent().is_some() {
            self.items.push(self.current_dir.join(".."));
        }

        let mut entries_vec = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.current_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if !self.show_hidden && file_name.starts_with('.') {
                    continue;
                }

                if path.is_dir() {
                    entries_vec.push(path);
                } else if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if matches!(ext_str.as_str(), "mp3" | "wav" | "ogg" | "flac") {
                        entries_vec.push(path);
                    }
                }
            }
        }
        // Sort directories first, then files
        entries_vec.sort_by(|a, b| {
            let a_is_dir = a.is_dir();
            let b_is_dir = b.is_dir();
            if a_is_dir && !b_is_dir {
                std::cmp::Ordering::Less
            } else if !a_is_dir && b_is_dir {
                std::cmp::Ordering::Greater
            } else {
                a.file_name().cmp(&b.file_name())
            }
        });

        self.items.extend(entries_vec);
        self.update_search_results();
        self.state.select(Some(0));
    }

    pub fn update_search_results(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_items = self.items.clone();
            self.filtered_radio_groups = self.radio_groups.clone();
        } else {
            let query = self.search_query.to_lowercase();
            // Filter files
            self.filtered_items = self
                .items
                .iter()
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_lowercase().contains(&query))
                        .unwrap_or(false)
                })
                .cloned()
                .collect();

            // Filter radio
            self.filtered_radio_groups = self
                .radio_groups
                .iter()
                .map(|g| {
                    let mut new_group = g.clone();
                    new_group
                        .stations
                        .retain(|s| s.name.to_lowercase().contains(&query));
                    new_group
                })
                .filter(|g| !g.stations.is_empty())
                .collect();
        }
    }

    pub fn on_search_input(&mut self, c: char) {
        self.search_query.push(c);
        self.update_search_results();
        // Reset selection
        match self.mode {
            AppMode::FileSystem => self.state.select(Some(0)),
            AppMode::Radio => self.radio_state.select(Some(0)),
            AppMode::Favorites => self.favorites_state.select(Some(0)),
            AppMode::Subsonic => self.subsonic_state.select(Some(0)),
        }
    }

    pub fn on_search_backspace(&mut self) {
        self.search_query.pop();
        self.update_search_results();
        // Reset selection
        match self.mode {
            AppMode::FileSystem => self.state.select(Some(0)),
            AppMode::Radio => self.radio_state.select(Some(0)),
            AppMode::Favorites => self.favorites_state.select(Some(0)),
            AppMode::Subsonic => self.subsonic_state.select(Some(0)),
        }
    }

    pub fn cancel_search(&mut self) {
        self.is_searching = false;
        self.search_query.clear();
        self.update_search_results();
        // Reset selection
        match self.mode {
            AppMode::FileSystem => self.state.select(Some(0)),
            AppMode::Radio => self.radio_state.select(Some(0)),
            AppMode::Favorites => self.favorites_state.select(Some(0)),
            AppMode::Subsonic => self.subsonic_state.select(Some(0)),
        }
    }

    pub fn submit_search(&mut self) {
        self.is_searching = false;
        // Keep the filter active
    }

    pub fn get_visible_radio_count(&self) -> usize {
        let mut count = 0;
        for group in &self.filtered_radio_groups {
            count += 1; // The group header
            if group.is_expanded {
                count += group.stations.len();
            }
        }
        count
    }

    pub fn get_radio_station_at_index(&self, index: usize) -> Option<&RadioStation> {
        let mut current_idx = 0;
        for group in &self.filtered_radio_groups {
            if current_idx == index {
                return None; // It's a group header
            }
            current_idx += 1;

            if group.is_expanded {
                if index < current_idx + group.stations.len() {
                    return Some(&group.stations[index - current_idx]);
                }
                current_idx += group.stations.len();
            }
        }
        None
    }

    pub fn next(&mut self) {
        match self.mode {
            AppMode::FileSystem => {
                let i = match self.state.selected() {
                    Some(i) => {
                        if i >= self.filtered_items.len().saturating_sub(1) {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.state.select(Some(i));
            }
            AppMode::Radio => {
                let count = self.get_visible_radio_count();
                let i = match self.radio_state.selected() {
                    Some(i) => {
                        if i >= count.saturating_sub(1) {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.radio_state.select(Some(i));
            }
            AppMode::Favorites => {
                let count = self.favorites.files.len() + self.favorites.stations.len();
                let i = match self.favorites_state.selected() {
                    Some(i) => {
                        if i >= count.saturating_sub(1) {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.favorites_state.select(Some(i));
            }
            AppMode::Subsonic => {
                let count = match self.subsonic_view {
                    SubsonicView::Servers => self.subsonic_clients.len(),
                    SubsonicView::Artists => self.subsonic_artists.len(),
                    SubsonicView::Albums(_) => self.subsonic_albums.len(),
                    SubsonicView::Tracks(_) => self.subsonic_tracks.len(),
                };
                let i = match self.subsonic_state.selected() {
                    Some(i) => {
                        if i >= count.saturating_sub(1) {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                self.subsonic_state.select(Some(i));
            }
        }
    }

    pub fn previous(&mut self) {
        match self.mode {
            AppMode::FileSystem => {
                let i = match self.state.selected() {
                    Some(i) => {
                        if i == 0 {
                            self.filtered_items.len().saturating_sub(1)
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.state.select(Some(i));
            }
            AppMode::Radio => {
                let count = self.get_visible_radio_count();
                let i = match self.radio_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            count.saturating_sub(1)
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.radio_state.select(Some(i));
            }
            AppMode::Favorites => {
                let count = self.favorites.files.len() + self.favorites.stations.len();
                let i = match self.favorites_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            count.saturating_sub(1)
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.favorites_state.select(Some(i));
            }
            AppMode::Subsonic => {
                let count = match self.subsonic_view {
                    SubsonicView::Servers => self.subsonic_clients.len(),
                    SubsonicView::Artists => self.subsonic_artists.len(),
                    SubsonicView::Albums(_) => self.subsonic_albums.len(),
                    SubsonicView::Tracks(_) => self.subsonic_tracks.len(),
                };
                let i = match self.subsonic_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            count.saturating_sub(1)
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                self.subsonic_state.select(Some(i));
            }
        }
    }

    pub fn get_selected_station(&self) -> Option<RadioStation> {
        let selected_idx = self.radio_state.selected().unwrap_or(0);
        let mut current_idx = 0;

        for group in &self.filtered_radio_groups {
            if current_idx == selected_idx {
                // It's a group header
                return None;
            }
            current_idx += 1;

            if group.is_expanded {
                if selected_idx < current_idx + group.stations.len() {
                    let station_idx = selected_idx - current_idx;
                    return Some(group.stations[station_idx].clone());
                }
                current_idx += group.stations.len();
            }
        }
        None
    }

    pub fn enter_directory(&mut self) {
        match self.mode {
            AppMode::FileSystem => {
                if let Some(path) = self
                    .state
                    .selected()
                    .and_then(|i| self.filtered_items.get(i))
                {
                    if path.ends_with("..") {
                        self.go_up();
                    } else if path.is_dir() {
                        self.current_dir = path.clone();
                        self.load_directory();
                    } else {
                        // Play music
                        self.play_file(path.clone());
                    }
                }
            }
            AppMode::Radio => {
                //Determine what action to take
                enum Action {
                    ToggleGroup(usize), // group_index
                    PlayStation(RadioStation),
                }

                let mut action = None;
                let selected_idx = self.radio_state.selected().unwrap_or(0);
                let mut current_idx = 0;

                for (g_idx, group) in self.filtered_radio_groups.iter().enumerate() {
                    if current_idx == selected_idx {
                        action = Some(Action::ToggleGroup(g_idx));
                        break;
                    }
                    current_idx += 1;

                    if group.is_expanded {
                        if selected_idx < current_idx + group.stations.len() {
                            let station_idx = selected_idx - current_idx;
                            action = Some(Action::PlayStation(group.stations[station_idx].clone()));
                            break;
                        }
                        current_idx += group.stations.len();
                    }
                }

                match action {
                    Some(Action::ToggleGroup(idx)) => {
                        // We need to find the corresponding group in the original list to toggle it
                        // Because filtered_radio_groups is derived
                        if let Some(filtered_group) = self.filtered_radio_groups.get(idx) {
                            let title = &filtered_group.title;
                            if let Some(original_group) =
                                self.radio_groups.iter_mut().find(|g| &g.title == title)
                            {
                                original_group.is_expanded = !original_group.is_expanded;
                            }
                        }
                        // Re-filter to update view
                        self.update_search_results();
                    }
                    Some(Action::PlayStation(station)) => {
                        self.play_radio(station);
                    }
                    None => {}
                }
            }
            AppMode::Favorites => {
                if let Some(i) = self.favorites_state.selected() {
                    if i < self.favorites.files.len() {
                        if let Some(path) = self.favorites.files.get(i) {
                            if path.is_dir() {
                                self.current_dir = path.clone();
                                self.mode = AppMode::FileSystem;
                                self.load_directory();
                            } else {
                                self.play_file(path.clone());
                            }
                        }
                    } else {
                        let station_idx = i - self.favorites.files.len();
                        if let Some(station) = self.favorites.stations.get(station_idx) {
                            self.play_radio(station.clone());
                        }
                    }
                }
            }
            AppMode::Subsonic => {
                if let Some(i) = self.subsonic_state.selected() {
                    if self.subsonic_clients.is_empty() {
                        return;
                    }

                    if let SubsonicView::Servers = self.subsonic_view {
                        self.active_subsonic_client = i;
                        let client = &self.subsonic_clients[i];
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        match rt.block_on(client.get_artists()) {
                            Ok(artists) => {
                                self.subsonic_artists = artists;
                                self.subsonic_view = SubsonicView::Artists;
                                self.subsonic_state.select(Some(0));
                            }
                            Err(e) => {
                                self.notification = Some((
                                    format!("Failed to load artists: {}", e),
                                    std::time::Instant::now(),
                                ));
                            }
                        }
                        return;
                    }

                    let client = &self.subsonic_clients[self.active_subsonic_client];
                    let rt = tokio::runtime::Runtime::new().unwrap();

                    match &self.subsonic_view {
                        SubsonicView::Servers => unreachable!(),
                        SubsonicView::Artists => {
                            if let Some(artist) = self.subsonic_artists.get(i) {
                                match rt.block_on(client.get_artist(&artist.id)) {
                                    Ok(albums) => {
                                        self.subsonic_albums = albums;
                                        self.subsonic_view =
                                            SubsonicView::Albums(artist.id.clone());
                                        self.subsonic_state.select(Some(0));
                                    }
                                    Err(e) => {
                                        self.last_error =
                                            Some(format!("Failed to load albums: {}", e));
                                    }
                                }
                            }
                        }
                        SubsonicView::Albums(_artist_id) => {
                            if let Some(album) = self.subsonic_albums.get(i) {
                                match rt.block_on(client.get_album(&album.id)) {
                                    Ok(tracks) => {
                                        self.subsonic_tracks = tracks;
                                        self.subsonic_view = SubsonicView::Tracks(album.id.clone());
                                        self.subsonic_state.select(Some(0));
                                    }
                                    Err(e) => {
                                        self.last_error =
                                            Some(format!("Failed to load tracks: {}", e));
                                    }
                                }
                            }
                        }
                        SubsonicView::Tracks(_album_id) => {
                            if let Some(track) = self.subsonic_tracks.get(i) {
                                let stream_url = client.get_stream_url(&track.id);
                                let title = if let Some(artist) = &track.artist {
                                    format!("{} - {}", artist, track.title)
                                } else {
                                    track.title.clone()
                                };
                                let station = crate::radio::RadioStation {
                                    name: title,
                                    url: stream_url,
                                    description: track.album.clone(),
                                    homepage: None,
                                    tags: None,
                                    last_playing: None,
                                };
                                self.play_radio(station);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn play_radio(&mut self, station: crate::radio::RadioStation) {
        self.current_track = Some(PathBuf::from(&station.name));
        self.is_paused = false;
        self.playback_start = Some(Instant::now());
        self.playback_elapsed = Duration::from_secs(0);
        self.track_duration = None;
        self.last_error = None;

        if let Some(sink) = &self.sink {
            sink.stop(); // Stop current track immediately

            let (tx, rx) = std::sync::mpsc::channel();
            self.source_receiver = Some(rx);

            let spectrum_data = self.spectrum_data.clone();
            let client = self.http_client.clone();
            let station_url = station.url.clone();

            // Spawn a thread to fetch the stream without blocking the UI or panicking tokio
            std::thread::spawn(move || {
                // Try to parse as playlist (PLS/M3U) first, if it fails, assume it's a direct stream URL
                let stream_url =
                    match crate::radio::fetch_playlist_stream_url(&client, &station_url) {
                        Ok(url) => url,
                        Err(_) => station_url, // Fallback to original URL
                    };

                match client.get(&stream_url).send() {
                    Ok(response) => {
                        if response.status().is_success() {
                            let reader = io::BufReader::new(response);
                            let source = Decoder::new(HttpStream::new(reader));
                            match source {
                                Ok(decoder) => {
                                    let source = decoder.convert_samples::<f32>();
                                    let sample_rate = source.sample_rate();

                                    let analyzer = AudioAnalyzer {
                                        input: source,
                                        buffer: Vec::with_capacity(2048),
                                        spectrum_data,
                                        sample_rate,
                                    };

                                    let _ = tx.send(Ok(Box::new(analyzer)));
                                }
                                Err(e) => {
                                    let _ = tx.send(Err(format!("Decoder error: {}", e)));
                                }
                            }
                        } else {
                            let _ = tx.send(Err(format!("HTTP error: {}", response.status())));
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(format!("Connection error: {}", e)));
                    }
                }
            });
        }
    }

    pub fn seek_to(&mut self, position: Duration) {
        if let Some(sink) = &self.sink {
            match sink.try_seek(position) {
                Ok(_) => {
                    self.playback_elapsed = position;
                    if !self.is_paused {
                        self.playback_start = Some(Instant::now());
                    }
                    self.update_mpris();
                }
                Err(e) => {
                    // Fallback for sources that don't support seeking (like some FLAC decoders)
                    if !self.seek_fallback(position) {
                        self.last_error = Some(format!("Seek error: {}", e));
                    }
                }
            }
        }
    }

    fn seek_fallback(&mut self, position: Duration) -> bool {
        if let Some(path) = &self.current_track
            && path.exists()
            && let Ok(file) = fs::File::open(path)
        {
            let reader = io::BufReader::new(file);
            if let Ok(decoder) = Decoder::new(reader) {
                let source = decoder.convert_samples::<f32>();
                let sample_rate = source.sample_rate();

                // Skip to position
                let source = source.skip_duration(position);

                let analyzer = AudioAnalyzer {
                    input: source,
                    buffer: Vec::with_capacity(2048),
                    spectrum_data: self.spectrum_data.clone(),
                    sample_rate,
                };

                if let Some(sink) = &self.sink {
                    sink.stop();
                    sink.append(analyzer);

                    if self.is_paused {
                        sink.pause();
                        self.playback_start = None;
                    } else {
                        sink.play();
                        self.playback_start = Some(Instant::now());
                    }
                }

                self.playback_elapsed = position;
                self.update_mpris();
                self.last_error = None;
                return true;
            }
        }
        false
    }

    pub fn handle_mpris_command(&mut self, cmd: MprisCommand) {
        match cmd {
            MprisCommand::Play => {
                if self.is_paused {
                    self.toggle_pause();
                }
            }
            MprisCommand::Pause => {
                if !self.is_paused {
                    self.toggle_pause();
                }
            }
            MprisCommand::PlayPause => self.toggle_pause(),
            MprisCommand::Stop => {
                if !self.is_paused {
                    self.toggle_pause();
                }
            }
            MprisCommand::Next => self.next_track(),
            MprisCommand::Previous => self.previous_track(),
            MprisCommand::Volume(vol) => {
                self.volume = vol as f32;
                if let Some(sink) = &self.sink {
                    sink.set_volume(self.volume);
                }
                self.update_mpris();
            }
            MprisCommand::Seek(offset_micros) => {
                let current_pos = self.playback_elapsed
                    + self
                        .playback_start
                        .map(|s| s.elapsed())
                        .unwrap_or(Duration::from_secs(0));

                let new_pos_micros = current_pos.as_micros() as i128 + offset_micros as i128;
                let new_pos = if new_pos_micros < 0 {
                    Duration::from_secs(0)
                } else {
                    let pos = Duration::from_micros(new_pos_micros as u64);
                    if let Some(duration) = self.track_duration {
                        if pos > duration { duration } else { pos }
                    } else {
                        pos
                    }
                };
                self.seek_to(new_pos);
            }
            MprisCommand::SetPosition(_track_id, position_micros) => {
                let mut pos = Duration::from_micros(position_micros as u64);
                if let Some(duration) = self.track_duration
                    && pos > duration
                {
                    pos = duration;
                }
                self.seek_to(pos);
            }
            MprisCommand::LoopStatus(status) => {
                self.loop_mode = match status {
                    mpris_server::LoopStatus::None => LoopMode::Off,
                    mpris_server::LoopStatus::Track => LoopMode::Track,
                    mpris_server::LoopStatus::Playlist => LoopMode::All,
                };
                self.update_mpris();
            }
        }
    }

    pub fn on_tick(&mut self) {
        // Check notification expiry
        if let Some((_, time)) = self.notification
            && time.elapsed() > Duration::from_secs(3)
        {
            self.notification = None;
        }

        // Check for updates
        if let Some(rx) = &self.update_receiver {
            match rx.try_recv() {
                Ok(Some(v)) => {
                    self.latest_version = Some(v);
                    self.update_receiver = None;
                }
                Ok(None) => {
                    self.update_receiver = None;
                }
                _ => {}
            }
        }

        // Check for station reload
        if let Some(rx) = &self.station_receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.station_receiver = None;
                    match result {
                        Ok(groups) => {
                            self.radio_groups = groups;
                            self.notification =
                                Some(("Stations reloaded".to_string(), Instant::now()));
                            self.update_search_results();
                        }
                        Err(e) => {
                            self.last_error = Some(format!("Failed to reload stations: {}", e));
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.station_receiver = None;
                }
            }
        }

        let source_result = self
            .source_receiver
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());

        if let Some(result) = source_result {
            match result {
                Ok(source) => {
                    if let Some(sink) = &self.sink {
                        sink.append(source);
                        sink.play();
                    }
                    self.last_error = None;
                }
                Err(e) => {
                    self.last_error = Some(e);
                    self.is_paused = true;
                    self.playback_start = None;
                }
            }
            self.source_receiver = None;
        }

        // Check for auto-advance
        if let Some(sink) = &self.sink
            && sink.empty()
            && !self.is_paused
            && self.current_track.is_some()
        {
            match self.loop_mode {
                LoopMode::All => self.next_track(),
                LoopMode::Track => {
                    if let Some(path) = self.current_track.clone() {
                        self.play_file(path);
                    }
                }
                LoopMode::Off => {}
            }
        }
        self.update_mpris();
    }

    pub fn update_mpris(&self) {
        if let Some(mpris_state) = &self.mpris_state {
            if let Ok(mut state) = mpris_state.lock() {
                state.playback_status = if self.current_track.is_some() {
                    if let Some(sink) = &self.sink {
                        if sink.empty() {
                            mpris_server::PlaybackStatus::Stopped
                        } else if self.is_paused {
                            mpris_server::PlaybackStatus::Paused
                        } else {
                            mpris_server::PlaybackStatus::Playing
                        }
                    } else {
                        mpris_server::PlaybackStatus::Stopped
                    }
                } else {
                    mpris_server::PlaybackStatus::Stopped
                };

                state.volume = self.volume as f64;
                state.loop_status = match self.loop_mode {
                    LoopMode::Off => mpris_server::LoopStatus::None,
                    LoopMode::Track => mpris_server::LoopStatus::Track,
                    LoopMode::All => mpris_server::LoopStatus::Playlist,
                };
                state.position = self.playback_elapsed;
                state.playback_start = if self.is_paused {
                    None
                } else {
                    self.playback_start
                };

                if let Some(path) = &self.current_track {
                    let title = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    state.title = title;
                    state.duration = self.track_duration;
                } else {
                    state.title = String::new();
                    state.duration = None;
                }
            }
            if let Some(notifier) = &self.mpris_notifier {
                notifier.send(()).ok();
            }
        }
    }

    pub fn play_file(&mut self, path: PathBuf) {
        self.current_track = Some(path.clone());
        self.is_paused = false;
        self.playback_start = Some(Instant::now());
        self.playback_elapsed = Duration::from_secs(0);
        self.track_duration = None;

        if let Some(sink) = &self.sink {
            if let Ok(file) = fs::File::open(&path) {
                let reader = io::BufReader::new(file);
                if let Ok(decoder) = Decoder::new(reader) {
                    self.track_duration = decoder.total_duration();

                    // Convert to f32 for analysis
                    let source = decoder.convert_samples::<f32>();
                    let sample_rate = source.sample_rate();

                    // Wrap in analyzer
                    let analyzer = AudioAnalyzer {
                        input: source,
                        buffer: Vec::with_capacity(2048),
                        spectrum_data: self.spectrum_data.clone(),
                        sample_rate,
                    };

                    sink.stop(); // Stop current track
                    sink.append(analyzer);
                    sink.play();
                }
            }
        } else {
            self.last_error = Some("Audio not available".to_string());
        }
        self.update_mpris();
    }

    pub fn toggle_pause(&mut self) {
        if let Some(sink) = &self.sink {
            if self.is_paused {
                sink.play();
                self.is_paused = false;
                self.playback_start = Some(Instant::now());
            } else {
                sink.pause();
                self.is_paused = true;
                if let Some(start) = self.playback_start {
                    self.playback_elapsed += start.elapsed();
                    self.playback_start = None;
                }
            }
        }
        self.update_mpris();
    }

    pub fn save_radio_station(&mut self) {
        if self.mode == AppMode::Radio
            && let Some(station) = self.get_selected_station()
        {
            if let Err(e) = crate::radio::add_station_to_config(&station) {
                // Ideally show error to user
                eprintln!("Failed to export station: {}", e);
            } else {
                self.notification = Some((format!("Exported: {}", station.name), Instant::now()));
            }
        }
    }

    pub fn save_config(&self) {
        if let Ok(mut config) = AppConfig::load() {
            config.volume = Some(self.volume);
            config.favorites = self.favorites.clone();
            if let Err(e) = config.save() {
                eprintln!("Failed to save config: {}", e);
            }
        }
    }

    pub fn change_volume(&mut self, delta: f32) {
        self.volume = (self.volume + delta).clamp(0.0, 1.0);
        if let Some(sink) = &self.sink {
            sink.set_volume(self.volume);
        }
        self.update_mpris();
    }

    pub fn toggle_loop(&mut self) {
        self.loop_mode = match self.loop_mode {
            LoopMode::Off => LoopMode::Track,
            LoopMode::Track => LoopMode::All,
            LoopMode::All => LoopMode::Off,
        };
    }

    pub fn open_add_modal(&mut self) {
        self.add_modal_state = Some(AddModalState::Selection);
    }

    pub fn handle_add_modal_input(&mut self, key: KeyCode) {
        if let Some(state) = &mut self.add_modal_state {
            match state {
                AddModalState::Selection => match key {
                    KeyCode::Char('s') => {
                        *state = AddModalState::InputStation {
                            name: String::new(),
                            url: String::new(),
                            description: String::new(),
                            homepage: String::new(),
                            tags: String::new(),
                            focused_field: 0,
                            original_url: None,
                        };
                    }
                    KeyCode::Char('r') => {
                        *state = AddModalState::InputSource {
                            title: String::new(),
                            json_url: String::new(),
                            container: String::new(),
                            map_name: String::new(),
                            map_url: String::new(),
                            map_desc: String::new(),
                            map_home: String::new(),
                            map_tags: String::new(),
                            focused_field: 0,
                            original_title: None,
                        };
                    }
                    KeyCode::Char('n') => {
                        *state = AddModalState::InputSubsonic {
                            server_url: String::new(),
                            username: String::new(),
                            password: String::new(),
                            focused_field: 0,
                            original_url: None,
                        };
                    }
                    KeyCode::Esc => self.add_modal_state = None,
                    _ => {}
                },
                AddModalState::InputStation {
                    name,
                    url,
                    description,
                    homepage,
                    tags,
                    focused_field,
                    original_url,
                } => match key {
                    KeyCode::Esc => self.add_modal_state = None,
                    KeyCode::Tab | KeyCode::Down => {
                        *focused_field = (*focused_field + 1) % 5;
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        *focused_field = (*focused_field + 4) % 5;
                    }
                    KeyCode::Enter => {
                        if name.trim().is_empty() || url.trim().is_empty() {
                            self.notification =
                                Some(("Name and URL are required".to_string(), Instant::now()));
                            return;
                        }
                        // Save
                        let station = crate::radio::RadioStation {
                            name: name.clone(),
                            url: url.clone(),
                            description: if description.is_empty() {
                                None
                            } else {
                                Some(description.clone())
                            },
                            homepage: if homepage.is_empty() {
                                None
                            } else {
                                Some(homepage.clone())
                            },
                            tags: if tags.is_empty() {
                                None
                            } else {
                                Some(tags.clone())
                            },
                            last_playing: None,
                        };

                        let result = if let Some(old_url) = original_url {
                            crate::radio::edit_station_in_config(old_url, &station)
                        } else {
                            crate::radio::add_station_to_config(&station)
                        };

                        if let Err(e) = result {
                            self.notification = Some((format!("Error: {}", e), Instant::now()));
                        } else {
                            self.notification =
                                Some((format!("Saved: {}", station.name), Instant::now()));
                            self.add_modal_state = None;
                            self.reload_stations();
                        }
                    }
                    KeyCode::Char(c) => {
                        let target = match *focused_field {
                            0 => name,
                            1 => url,
                            2 => description,
                            3 => homepage,
                            4 => tags,
                            _ => return,
                        };
                        target.push(c);
                    }
                    KeyCode::Backspace => {
                        let target = match *focused_field {
                            0 => name,
                            1 => url,
                            2 => description,
                            3 => homepage,
                            4 => tags,
                            _ => return,
                        };
                        target.pop();
                    }
                    _ => {}
                },
                AddModalState::InputSource {
                    title,
                    json_url,
                    container,
                    map_name,
                    map_url,
                    map_desc,
                    map_home,
                    map_tags,
                    focused_field,
                    original_title,
                } => match key {
                    KeyCode::Esc => self.add_modal_state = None,
                    KeyCode::Tab | KeyCode::Down => {
                        *focused_field = (*focused_field + 1) % 8;
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        *focused_field = (*focused_field + 7) % 8;
                    }
                    KeyCode::Enter => {
                        if title.trim().is_empty()
                            || json_url.trim().is_empty()
                            || map_name.trim().is_empty()
                            || map_url.trim().is_empty()
                        {
                            self.notification = Some((
                                "Title, JSON URL, Name Map, and URL Map are required".to_string(),
                                Instant::now(),
                            ));
                            return;
                        }
                        // Save
                        let source = crate::config::RadioSourceConfig {
                            title: title.clone(),
                            json_url: json_url.clone(),
                            container: if container.is_empty() {
                                None
                            } else {
                                Some(container.clone())
                            },
                            mapping: crate::config::StationMapping {
                                station_name: map_name.clone(),
                                station_url: map_url.clone(),
                                description: if map_desc.is_empty() {
                                    None
                                } else {
                                    Some(map_desc.clone())
                                },
                                homepage: if map_home.is_empty() {
                                    None
                                } else {
                                    Some(map_home.clone())
                                },
                                tags: if map_tags.is_empty() {
                                    None
                                } else {
                                    Some(map_tags.clone())
                                },
                                last_playing: None,
                            },
                        };

                        let result = if let Some(old_title) = original_title {
                            crate::radio::edit_source_in_config(old_title, &source)
                        } else {
                            crate::radio::add_source_to_config(&source)
                        };

                        if let Err(e) = result {
                            self.notification = Some((format!("Error: {}", e), Instant::now()));
                        } else {
                            self.notification =
                                Some((format!("Saved Source: {}", source.title), Instant::now()));
                            // Invalidate cache for this source to ensure fresh fetch
                            crate::radio::invalidate_source_cache(&source.title, None);

                            self.add_modal_state = None;
                            self.reload_stations();
                        }
                    }
                    KeyCode::Char(c) => {
                        let target = match *focused_field {
                            0 => title,
                            1 => json_url,
                            2 => container,
                            3 => map_name,
                            4 => map_url,
                            5 => map_desc,
                            6 => map_home,
                            7 => map_tags,
                            _ => return,
                        };
                        target.push(c);
                    }
                    KeyCode::Backspace => {
                        let target = match *focused_field {
                            0 => title,
                            1 => json_url,
                            2 => container,
                            3 => map_name,
                            4 => map_url,
                            5 => map_desc,
                            6 => map_home,
                            7 => map_tags,
                            _ => return,
                        };
                        target.pop();
                    }
                    _ => {}
                },
                AddModalState::InputSubsonic {
                    server_url,
                    username,
                    password,
                    focused_field,
                    original_url,
                } => match key {
                    KeyCode::Esc => self.add_modal_state = None,
                    KeyCode::Tab | KeyCode::Down => {
                        *focused_field = (*focused_field + 1) % 3;
                    }
                    KeyCode::BackTab | KeyCode::Up => {
                        *focused_field = if *focused_field > 0 {
                            *focused_field - 1
                        } else {
                            2
                        };
                    }
                    KeyCode::Enter => {
                        if server_url.is_empty() || username.is_empty() {
                            self.notification = Some((
                                "Server URL and Username are required".to_string(),
                                std::time::Instant::now(),
                            ));
                            return;
                        }
                        let mut config = crate::config::AppConfig::load().unwrap_or_default();
                        let nav_source = crate::config::SubsonicSourceConfig {
                            server_url: server_url.clone(),
                            username: username.clone(),
                            password: if password.is_empty() {
                                None
                            } else {
                                Some(password.clone())
                            },
                            auth_token: None,
                        };

                        if let Some(old_url) = original_url {
                            if let Some(nav_config) = &mut config.subsonic
                                && let Some(idx) = nav_config
                                    .sources
                                    .iter()
                                    .position(|s| s.server_url == *old_url)
                            {
                                nav_config.sources[idx] = nav_source;
                            }
                        } else {
                            if config.subsonic.is_none() {
                                config.subsonic = Some(crate::config::SubsonicConfig::default());
                            }
                            if let Some(nav_config) = &mut config.subsonic {
                                nav_config.sources.push(nav_source);
                            }
                        }

                        if let Err(e) = config.save() {
                            self.notification =
                                Some((format!("Error: {}", e), std::time::Instant::now()));
                        } else {
                            self.notification = Some((
                                format!("Saved Subsonic: {}", server_url),
                                std::time::Instant::now(),
                            ));

                            self.reload_subsonic();
                            self.add_modal_state = None;
                        }
                    }
                    KeyCode::Char(c) => {
                        let target = match *focused_field {
                            0 => server_url,
                            1 => username,
                            2 => password,
                            _ => return,
                        };
                        target.push(c);
                    }
                    KeyCode::Backspace => {
                        let target = match *focused_field {
                            0 => server_url,
                            1 => username,
                            2 => password,
                            _ => return,
                        };
                        target.pop();
                    }
                    _ => {}
                },
                AddModalState::Confirmation {
                    message: _,
                    context,
                } => match key {
                    KeyCode::Char('y') | KeyCode::Enter => {
                        // Clone context to avoid borrow checker issues if we need to use self
                        let ctx = match context {
                            ConfirmationContext::DeleteStation(url) => {
                                ConfirmationContext::DeleteStation(url.clone())
                            }
                            ConfirmationContext::DeleteSource(title) => {
                                ConfirmationContext::DeleteSource(title.clone())
                            }
                            ConfirmationContext::DeleteSubsonic(url) => {
                                ConfirmationContext::DeleteSubsonic(url.clone())
                            }
                        };
                        self.confirm_delete(&ctx);
                    }
                    KeyCode::Char('n') | KeyCode::Esc => {
                        self.add_modal_state = None;
                    }
                    _ => {}
                },
            }
        }
    }

    pub fn open_delete_modal(&mut self) {
        match self.mode {
            AppMode::Radio => {
                let selected_idx = self.radio_state.selected().unwrap_or(0);
                let mut current_idx = 0;

                for group in &self.filtered_radio_groups {
                    // Determine if the group header is selected
                    if current_idx == selected_idx {
                        if group.title != "Custom Stations"
                            && let Ok(config) = crate::radio::load_config()
                            && let Some(source) =
                                config.sources.iter().find(|s| s.title == group.title)
                        {
                            self.add_modal_state = Some(AddModalState::Confirmation {
                                message: format!(
                                    "Are you sure you want to delete source '{}'?",
                                    source.title
                                ),
                                context: ConfirmationContext::DeleteSource(source.title.clone()),
                            });
                        }
                        return;
                    }
                    current_idx += 1;

                    if group.is_expanded {
                        // Determine if a station within this group is selected
                        if selected_idx < current_idx + group.stations.len() {
                            let station_idx = selected_idx - current_idx;
                            let station = &group.stations[station_idx];

                            // Only allow deleting if it's in the "Custom Stations" group
                            if group.title == "Custom Stations"
                                && let Ok(config) = crate::radio::load_config()
                                && let Some(s) = config
                                    .individual_stations
                                    .iter()
                                    .find(|s| s.station_url == station.url)
                            {
                                self.add_modal_state = Some(AddModalState::Confirmation {
                                    message: format!(
                                        "Are you sure you want to delete station '{}'?",
                                        s.name
                                    ),
                                    context: ConfirmationContext::DeleteStation(
                                        s.station_url.clone(),
                                    ),
                                });
                            }
                            return;
                        }
                        current_idx += group.stations.len();
                    }
                }
            }
            AppMode::Subsonic => {
                if let SubsonicView::Servers = self.subsonic_view
                    && let Some(i) = self.subsonic_state.selected()
                    && i < self.subsonic_clients.len()
                {
                    let client = &self.subsonic_clients[i];
                    self.add_modal_state = Some(AddModalState::Confirmation {
                        message: format!(
                            "Are you sure you want to delete Subsonic Server '{}'?",
                            client.config.server_url
                        ),
                        context: ConfirmationContext::DeleteSubsonic(
                            client.config.server_url.clone(),
                        ),
                    });
                }
            }
            _ => (),
        }
    }

    fn confirm_delete(&mut self, context: &ConfirmationContext) {
        let result = match context {
            ConfirmationContext::DeleteStation(url) => {
                crate::radio::delete_station_from_config(url)
            }
            ConfirmationContext::DeleteSource(title) => {
                crate::radio::delete_source_from_config(title)
            }
            ConfirmationContext::DeleteSubsonic(server_url) => {
                crate::config::delete_subsonic_from_config(server_url)
            }
        };

        if let Err(e) = result {
            self.notification = Some((format!("Error: {}", e), Instant::now()));
        } else {
            self.notification = Some(("Deleted successfully".to_string(), Instant::now()));
            self.add_modal_state = None;
            self.reload_stations();
            if matches!(context, ConfirmationContext::DeleteSubsonic(_)) {
                self.reload_subsonic();
            }
        }
    }

    pub fn reload_stations(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.station_receiver = Some(rx);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let result = rt.block_on(crate::radio::fetch_all_stations(None, false, true));
            let _ = tx.send(result.map_err(|e| e.to_string()));
        });
    }

    pub fn reload_subsonic(&mut self) {
        if let Ok(config) = crate::config::AppConfig::load() {
            if let Some(subsonic) = config.subsonic {
                self.subsonic_clients = subsonic
                    .sources
                    .into_iter()
                    .map(crate::subsonic::SubsonicClient::new)
                    .collect();
            } else {
                self.subsonic_clients.clear();
            }

            if self.subsonic_clients.is_empty() {
                self.subsonic_artists.clear();
                self.subsonic_albums.clear();
                self.subsonic_tracks.clear();
                self.subsonic_state.select(None);
            } else {
                self.subsonic_state.select(Some(0));

                // Fetch the new lib
                let rt = tokio::runtime::Runtime::new().unwrap();
                match rt.block_on(self.subsonic_clients[0].get_artists()) {
                    Ok(artists) => {
                        self.subsonic_artists = artists;
                    }
                    Err(e) => {
                        self.notification = Some((
                            format!("Failed to load Subsonic library: {}", e),
                            Instant::now(),
                        ));
                    }
                }
            }
        }
    }

    pub fn open_edit_modal(&mut self) {
        match self.mode {
            AppMode::Radio => {
                let selected_idx = self.radio_state.selected().unwrap_or(0);
                let mut current_idx = 0;

                for group in &self.filtered_radio_groups {
                    // Determine if the group header is selected
                    if current_idx == selected_idx {
                        if group.title != "Custom Stations"
                            && let Ok(config) = crate::radio::load_config()
                            && let Some(source) =
                                config.sources.iter().find(|s| s.title == group.title)
                        {
                            self.add_modal_state = Some(AddModalState::InputSource {
                                title: source.title.clone(),
                                json_url: source.json_url.clone(),
                                container: source.container.clone().unwrap_or_default(),
                                map_name: source.mapping.station_name.clone(),
                                map_url: source.mapping.station_url.clone(),
                                map_desc: source.mapping.description.clone().unwrap_or_default(),
                                map_home: source.mapping.homepage.clone().unwrap_or_default(),
                                map_tags: source.mapping.tags.clone().unwrap_or_default(),
                                focused_field: 0,
                                original_title: Some(source.title.clone()),
                            });
                        }
                        return;
                    }
                    current_idx += 1;

                    if group.is_expanded {
                        // Determine if a station within this group is selected
                        if selected_idx < current_idx + group.stations.len() {
                            let station_idx = selected_idx - current_idx;
                            let station = &group.stations[station_idx];

                            // Only allow editing if it's in the "Custom Stations" group
                            if group.title == "Custom Stations"
                                && let Ok(config) = crate::radio::load_config()
                                && let Some(s) = config
                                    .individual_stations
                                    .iter()
                                    .find(|s| s.station_url == station.url)
                            {
                                self.add_modal_state = Some(AddModalState::InputStation {
                                    name: s.name.clone(),
                                    url: s.station_url.clone(),
                                    description: s.description.clone().unwrap_or_default(),
                                    homepage: s.homepage.clone().unwrap_or_default(),
                                    tags: s.tags.clone().unwrap_or_default(),
                                    focused_field: 0,
                                    original_url: Some(s.station_url.clone()),
                                });
                            }
                            return;
                        }
                        current_idx += group.stations.len();
                    }
                }
            }
            AppMode::Subsonic => {
                if let SubsonicView::Servers = self.subsonic_view
                    && let Some(i) = self.subsonic_state.selected()
                    && i < self.subsonic_clients.len()
                {
                    let client = &self.subsonic_clients[i];
                    self.add_modal_state = Some(AddModalState::InputSubsonic {
                        server_url: client.config.server_url.clone(),
                        username: client.config.username.clone(),
                        password: client.config.password.clone().unwrap_or_default(),
                        focused_field: 0,
                        original_url: Some(client.config.server_url.clone()),
                    });
                }
            }
            _ => (),
        }
    }

    pub fn toggle_favorite(&mut self) {
        match self.mode {
            AppMode::FileSystem => {
                if let Some(path) = self.state.selected().and_then(|i| self.items.get(i))
                    && path.file_name().and_then(|n| n.to_str()) != Some("..")
                {
                    self.favorites.toggle_file(path.clone());
                    self.save_config();
                }
            }
            AppMode::Radio => {
                if let Some(i) = self.radio_state.selected()
                    && let Some(station) = self.get_radio_station_at_index(i)
                {
                    self.favorites.toggle_station(station.clone());
                    self.save_config();
                }
            }
            AppMode::Favorites => {
                // In favorites mode, 'f' could remove the selected item
                if let Some(i) = self.favorites_state.selected() {
                    if i < self.favorites.files.len() {
                        if let Some(path) = self.favorites.files.get(i) {
                            self.favorites.toggle_file(path.clone());
                            self.save_config();
                        }
                    } else {
                        let station_idx = i - self.favorites.files.len();
                        if let Some(station) = self.favorites.stations.get(station_idx) {
                            self.favorites.toggle_station(station.clone());
                            self.save_config();
                        }
                    }
                    // Adjust selection if needed
                    let count = self.favorites.files.len() + self.favorites.stations.len();
                    if count == 0 {
                        self.favorites_state.select(None);
                    } else if i >= count {
                        self.favorites_state.select(Some(count - 1));
                    }
                }
            }
            AppMode::Subsonic => {
                // Not supported yet
            }
        }
    }

    pub fn next_track(&mut self) {
        // Only for FileSystem mode
        if matches!(self.mode, AppMode::Radio) {
            return;
        }

        // Find current track index in filtered items
        if let Some(idx) = self
            .current_track
            .as_ref()
            .and_then(|cp| self.filtered_items.iter().position(|p| p == cp))
        {
            // Find next playable file
            for i in (idx + 1)..self.filtered_items.len() {
                let path = &self.filtered_items[i];
                if !path.is_dir() && path.file_name().and_then(|n| n.to_str()) != Some("..") {
                    self.play_file(path.clone());
                    self.state.select(Some(i)); // Move selection to playing file
                    return;
                }
            }

            // If LoopMode::All, wrap around
            if matches!(self.loop_mode, LoopMode::All) {
                for i in 0..=idx {
                    let path = &self.filtered_items[i];
                    if !path.is_dir() && path.file_name().and_then(|n| n.to_str()) != Some("..") {
                        self.play_file(path.clone());
                        self.state.select(Some(i)); // Move selection to playing file
                        return;
                    }
                }
            }
        }
    }

    pub fn previous_track(&mut self) {
        // Only for FileSystem mode
        if matches!(self.mode, AppMode::Radio) {
            return;
        }

        if let Some(idx) = self
            .current_track
            .as_ref()
            .and_then(|cp| self.filtered_items.iter().position(|p| p == cp))
        {
            // Find previous playable file
            for i in (0..idx).rev() {
                let path = &self.filtered_items[i];
                if !path.is_dir() && path.file_name().and_then(|n| n.to_str()) != Some("..") {
                    self.play_file(path.clone());
                    self.state.select(Some(i));
                    return;
                }
            }
        }
    }

    pub fn go_up(&mut self) {
        match self.mode {
            AppMode::FileSystem => {
                if let Some(parent) = self.current_dir.parent() {
                    self.current_dir = parent.to_path_buf();
                    self.load_directory();
                }
            }
            AppMode::Subsonic => match self.subsonic_view {
                SubsonicView::Servers => {}
                SubsonicView::Artists => {
                    self.subsonic_view = SubsonicView::Servers;
                    self.subsonic_state
                        .select(Some(self.active_subsonic_client));
                }
                SubsonicView::Albums(_) => {
                    self.subsonic_view = SubsonicView::Artists;
                    self.subsonic_state.select(Some(0));
                }
                SubsonicView::Tracks(ref artist_id) => {
                    self.subsonic_view = SubsonicView::Albums(artist_id.clone());
                    self.subsonic_state.select(Some(0));
                }
            },
            _ => {}
        }
    }
}

struct HttpStream<R> {
    inner: R,
    buffer: Vec<u8>,
    pos: u64,
    dropped_bytes: u64,
}

impl<R: io::Read> HttpStream<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
            pos: 0,
            dropped_bytes: 0,
        }
    }
}

impl<R: io::Read> io::Read for HttpStream<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos < self.buffer.len() as u64 {
            let start = self.pos as usize;
            let available = self.buffer.len() - start;
            let to_read = std::cmp::min(buf.len(), available);
            buf[0..to_read].copy_from_slice(&self.buffer[start..start + to_read]);
            self.pos += to_read as u64;
            return Ok(to_read);
        }

        let n = self.inner.read(buf)?;
        if n > 0 {
            // Increase buffer to 2MB to handle longer probes
            if self.buffer.len() < 2 * 1024 * 1024 {
                let space = 2 * 1024 * 1024 - self.buffer.len();
                let to_buffer = std::cmp::min(n, space);
                self.buffer.extend_from_slice(&buf[0..to_buffer]);
                if n > space {
                    self.dropped_bytes += (n - space) as u64;
                }
            } else {
                self.dropped_bytes += n as u64;
            }
            self.pos += n as u64;
        }
        Ok(n)
    }
}

impl<R: io::Read> io::Seek for HttpStream<R> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            io::SeekFrom::Start(p) => p,
            io::SeekFrom::Current(d) => (self.pos as i64 + d) as u64,
            io::SeekFrom::End(_) => {
                return Err(io::Error::other("Cannot seek from end"));
            }
        };

        if new_pos <= self.buffer.len() as u64 {
            if self.dropped_bytes > 0 {
                return Err(io::Error::other("Cannot seek: buffer overflowed"));
            }
            self.pos = new_pos;
            Ok(new_pos)
        } else if new_pos == self.pos {
            Ok(new_pos)
        } else {
            Err(io::Error::other("Cannot seek in unbuffered region"))
        }
    }
}

pub trait EventSource {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;
    fn read(&mut self) -> io::Result<Event>;
}

pub struct CrosstermEvents;
impl EventSource for CrosstermEvents {
    fn poll(&mut self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }
    fn read(&mut self) -> io::Result<Event> {
        event::read()
    }
}

pub fn run_app<B: Backend, E: EventSource>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    events: &mut E,
    mpris_rx: &Receiver<MprisCommand>,
) -> io::Result<()> {
    loop {
        app.on_tick();

        // Handle MPRIS commands
        while let Ok(cmd) = mpris_rx.try_recv() {
            app.handle_mpris_command(cmd);
        }

        terminal.draw(|f| ui::draw(f, app))?;

        if events.poll(Duration::from_millis(50))? {
            let event = events.read()?;
            if let Event::Key(key) = event {
                if key.kind == KeyEventKind::Release {
                    continue;
                }

                let is_repeat = key.kind == KeyEventKind::Repeat;

                // Check if we are in modal state and if we should process the key
                let (in_modal, modal_should_process) = if let Some(state) = &app.add_modal_state {
                    (
                        true,
                        !is_repeat
                            || match state {
                                AddModalState::Selection => false,
                                AddModalState::InputStation { .. }
                                | AddModalState::InputSource { .. }
                                | AddModalState::InputSubsonic { .. } => {
                                    matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace)
                                }
                                AddModalState::Confirmation { .. } => false,
                            },
                    )
                } else {
                    (false, false)
                };

                if in_modal {
                    if modal_should_process {
                        app.handle_add_modal_input(key.code);
                    }
                } else if app.is_searching {
                    match key.code {
                        KeyCode::Esc => {
                            if !is_repeat {
                                app.cancel_search()
                            }
                        }
                        KeyCode::Enter => {
                            if !is_repeat {
                                app.submit_search()
                            }
                        }
                        KeyCode::Backspace => app.on_search_backspace(),
                        KeyCode::Char(c) => app.on_search_input(c),
                        _ => {}
                    }
                } else if app.show_help {
                    match key.code {
                        KeyCode::Char('?') | KeyCode::Esc => {
                            if !is_repeat {
                                app.show_help = false
                            }
                        }
                        KeyCode::Char('q') => return Ok(()),
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('?') => {
                            if !is_repeat {
                                app.show_help = true
                            }
                        }
                        KeyCode::Char('/') => {
                            if !is_repeat {
                                app.is_searching = true;
                                app.search_query.clear();
                                app.update_search_results();
                            }
                        }
                        KeyCode::Esc => {
                            if !is_repeat && !app.search_query.is_empty() {
                                app.cancel_search();
                            }
                        }
                        KeyCode::Char('h') => {
                            if !is_repeat {
                                app.show_hidden = !app.show_hidden;
                                app.load_directory();
                            }
                        }
                        KeyCode::Tab => {
                            if !is_repeat {
                                app.mode = match app.mode {
                                    AppMode::FileSystem => AppMode::Radio,
                                    AppMode::Radio => AppMode::Favorites,
                                    AppMode::Favorites => {
                                        if app.subsonic_clients.is_empty() {
                                            AppMode::FileSystem
                                        } else {
                                            AppMode::Subsonic
                                        }
                                    }
                                    AppMode::Subsonic => AppMode::FileSystem,
                                };
                                // Reset search when switching modes.
                                app.cancel_search();
                            }
                        }
                        KeyCode::Char('j') | KeyCode::Down => app.next(),
                        KeyCode::Char('k') | KeyCode::Up => app.previous(),
                        KeyCode::Char('+') | KeyCode::Char('=') => app.change_volume(0.05),
                        KeyCode::Char('l') => {
                            if !is_repeat {
                                app.toggle_loop()
                            }
                        }
                        KeyCode::Char('f') => {
                            if !is_repeat {
                                app.toggle_favorite()
                            }
                        }
                        KeyCode::Char('-') => app.change_volume(-0.05),
                        KeyCode::Left => app.previous_track(),
                        KeyCode::Right => app.next_track(),
                        KeyCode::Char('x') => {
                            if !is_repeat {
                                app.save_radio_station()
                            }
                        }
                        KeyCode::Char('a') => {
                            if !is_repeat {
                                app.open_add_modal()
                            }
                        }
                        KeyCode::Char('e') => {
                            if !is_repeat {
                                app.open_edit_modal()
                            }
                        }
                        KeyCode::Char(' ') => {
                            if !is_repeat {
                                app.toggle_pause()
                            }
                        }
                        KeyCode::Enter => {
                            if !is_repeat {
                                app.enter_directory()
                            }
                        }
                        KeyCode::Backspace | KeyCode::Delete => {
                            if app.mode == AppMode::Radio
                                || (app.mode == AppMode::Subsonic
                                    && app.subsonic_view == SubsonicView::Servers)
                            {
                                if !is_repeat {
                                    app.open_delete_modal();
                                }
                            } else if key.code == KeyCode::Backspace {
                                app.go_up();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_delete;
#[cfg(test)]
mod tests_edit;
