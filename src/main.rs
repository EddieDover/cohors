mod app;
mod audio;
mod favorites;
mod mpris;
mod radio;
mod ui;

use anyhow::Result;
use app::App;
use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Set the default volume (0-100)
    #[arg(short, long)]
    volume: Option<u8>,

    /// Start in radio mode
    #[arg(short, long)]
    radio: bool,

    /// Path to the station configuration file
    #[arg(short = 's', long = "station-file")]
    station_file: Option<String>,

    /// Invalidate the station cache
    #[arg(long = "invalidate-cache")]
    invalidate_cache: bool,

    /// Path to a file or directory to play
    #[arg(num_args(0..))]
    path: Vec<String>,
}

#[cfg(unix)]
fn redirect_stderr() -> Result<()> {
    let mut log_path = std::env::temp_dir();
    log_path.push("cohors_stderr.log");
    let file = std::fs::File::create(&log_path)?;
    let fd = file.as_raw_fd();
    unsafe {
        libc::dup2(fd, 2);
    }
    Ok(())
}

#[cfg(not(unix))]
fn redirect_stderr() -> Result<()> {
    Ok(())
}

fn apply_args(app: &mut App, args: Args) {
    // Apply volume
    if let Some(vol) = args.volume {
        let vol_f32 = vol as f32 / 100.0;
        app.volume = vol_f32.clamp(0.0, 1.0);
        if let Some(sink) = &app.sink {
            sink.set_volume(app.volume);
        }
    }

    // Apply radio mode
    if args.radio {
        app.mode = app::AppMode::Radio;
    }

    // Apply path
    if !args.path.is_empty() {
        let path_str = args.path.join(" ");
        let path = PathBuf::from(path_str);
        let path = path.canonicalize().unwrap_or(path);
        if path.is_dir() {
            app.current_dir = path;
            app.load_directory();
            app.loop_mode = app::LoopMode::All;

            // Find first playable file
            let first_file = app.items.iter().find(|p| p.is_file()).cloned();
            if let Some(first_file) = first_file {
                app.play_file(first_file.clone());
                if let Some(idx) = app.items.iter().position(|x| x == &first_file) {
                    app.state.select(Some(idx));
                }
            }
        } else if path.is_file() {
            if let Some(parent) = path.parent() {
                app.current_dir = parent.to_path_buf();
                app.load_directory();
            }
            app.play_file(path.clone());
            if let Some(idx) = app.items.iter().position(|x| x == &path) {
                app.state.select(Some(idx));
            }
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Create app state first (to handle audio init noise before TUI)
    let mut app = App::new();

    // Setup MPRIS
    let (tx, rx) = std::sync::mpsc::channel();
    let mpris_state = std::sync::Arc::new(std::sync::Mutex::new(mpris::MprisState::default()));
    app.mpris_state = Some(mpris_state.clone());

    let (notifier_tx, notifier_rx) = tokio::sync::mpsc::unbounded_channel();
    app.mpris_notifier = Some(notifier_tx);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Ok(_handler) = mpris::MprisHandler::new(tx, mpris_state, notifier_rx).await {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                }
            }
        });
    });

    apply_args(&mut app, args.clone());

    // Fetch radio stations
    println!("Loading radio stations...");
    let rt = tokio::runtime::Runtime::new()?;
    let station_file = args.station_file.map(PathBuf::from);
    match rt.block_on(radio::fetch_all_stations(
        station_file,
        None,
        args.invalidate_cache,
    )) {
        Ok(groups) => {
            println!("Loaded {} groups", groups.len());
            app.radio_groups = groups;
            app.update_search_results(); // Initialize filtered groups
            app.radio_state.select(Some(0));
        }
        Err(e) => {
            eprintln!("Failed to load radio stations: {}", e);
        }
    }
    // Wait a bit to see the message before TUI starts
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Redirect stderr to avoid TUI corruption
    if let Err(e) = redirect_stderr() {
        eprintln!("Failed to redirect stderr: {}", e);
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let mut events = app::CrosstermEvents;
    let res = app::run_app(&mut terminal, &mut app, &mut events, &rx);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

#[cfg(test)]
mod tests;
