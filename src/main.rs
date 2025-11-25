mod app;
mod audio;
mod ui;
mod radio;

use std::io;
use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Create app state first (to handle audio init noise before TUI)
    let mut app = App::new();

    // Fetch radio stations
    if let Ok(channels) = radio::fetch_channels().await {
        app.radio_stations = channels;
        app.radio_state.select(Some(0));
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let mut events = app::CrosstermEvents;
    let res = app::run_app(&mut terminal, &mut app, &mut events);

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

