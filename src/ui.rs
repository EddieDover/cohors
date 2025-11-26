use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{BarChart, Block, Borders, Gauge, List, ListItem, Paragraph, Clear},
    Frame,
};
use std::time::Duration;
use crate::app::{App, AppMode};

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    
    // Vertical split: Top (Main), Status (4), Help (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0), 
            Constraint::Length(4),
            Constraint::Length(1)
        ].as_ref())
        .split(size);

    // Bottom Panel
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
        .split(chunks[1]);

    // Left Bottom: Status & Progress
    let status_block = Block::default().borders(Borders::ALL).title("Status");
    let status_area = bottom_chunks[0];
    let status_inner_area = status_block.inner(status_area);
    
    f.render_widget(status_block, status_area);
    
    let status_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(status_inner_area);

    let (status_text, ratio, label) = if let Some(err) = &app.last_error {
        (format!("Error: {}", err), 0.0, String::new())
    } else if let Some(path) = &app.current_track {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let state = if app.is_paused { "Paused" } else { "Playing" };
        
        // Calculate time
        let mut elapsed = app.playback_elapsed;
        if let Some(start) = app.playback_start.filter(|_| !app.is_paused) {
            elapsed += start.elapsed();
        }
        
        let total = app.track_duration.unwrap_or(Duration::from_secs(0));
        let elapsed_secs = elapsed.as_secs();
        let total_secs = total.as_secs();
        
        let ratio = if total_secs > 0 {
            (elapsed_secs as f64 / total_secs as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let time_str = if total_secs > 0 {
            format!("{:02}:{:02} / {:02}:{:02}", 
                elapsed_secs / 60, elapsed_secs % 60,
                total_secs / 60, total_secs % 60)
        } else {
            format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60)
        };

        (
            format!("{}: {} (Vol: {:.0}%)", state, name, app.volume * 100.0),
            ratio,
            time_str
        )
    } else {
        ("Playing: <Nothing>".to_string(), 0.0, String::new())
    };
    
    let status_paragraph = Paragraph::new(status_text);
    f.render_widget(status_paragraph, status_layout[0]);
    
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
        .ratio(ratio)
        .label(label);
    f.render_widget(gauge, status_layout[1]);

    // Right Bottom: Visualizer (Real)
    let vis_area = bottom_chunks[1];
    let vis_block = Block::default().borders(Borders::ALL).title("Visualizer");
    
    let mut data: Vec<(&str, u64)> = Vec::new();
    if let Ok(spectrum) = app.spectrum_data.lock() {
        data = spectrum.clone();
    }
    
    // If empty or paused, show flat line
    if data.is_empty() || app.is_paused || app.current_track.is_none() {
        data = vec![
            ("Sub", 0), ("Bass", 0), ("LowM", 0), ("Mid", 0),
            ("HighM", 0), ("Pres", 0), ("Bril", 0), ("Air", 0)
        ];
    }

    // Calculate dynamic bar width to fill the container
    let inner_width = vis_area.width.saturating_sub(2); // Subtract borders
    let num_bars = data.len() as u16;
    let bar_gap = 1;
    let total_gap = bar_gap * (num_bars.saturating_sub(1));
    let bar_width = if num_bars > 0 {
        (inner_width.saturating_sub(total_gap)) / num_bars
    } else {
        3
    };

    let barchart = BarChart::default()
        .block(vis_block)
        .data(&data)
        .bar_width(bar_width)
        .bar_gap(bar_gap)
        .bar_style(Style::default().fg(Color::Cyan));
    
    f.render_widget(barchart, vis_area);

    // Top Panel Content
    match app.mode {
        AppMode::FileSystem => {
            let top_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[0]);

            // Left Sidebar: File List
            let items: Vec<ListItem> = app
                .items
                .iter()
                .map(|path| {
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    let style = if path.is_dir() {
                        Style::default().fg(Color::Blue)
                    } else if Some(path) == app.current_track.as_ref() {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(name).style(style)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Files"))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))
                .highlight_symbol(">> ");
            
            f.render_stateful_widget(list, top_chunks[0], &mut app.state);

            // Right Panel: Info
            let info_text = if let Some(i) = app.state.selected() {
                if let Some(path) = app.items.get(i) {
                    format!("Selected: {}\nPath: {}", path.file_name().unwrap_or_default().to_string_lossy(), path.display())
                } else {
                    "No selection".to_string()
                }
            } else {
                "No selection".to_string()
            };
            
            let info_block = Block::default().borders(Borders::ALL).title("Info");
            let info_paragraph = Paragraph::new(info_text).block(info_block);
            f.render_widget(info_paragraph, top_chunks[1]);
        }
        AppMode::Radio => {
            let top_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[0]);

            let items: Vec<ListItem> = app
                .radio_stations
                .iter()
                .map(|channel| {
                    ListItem::new(channel.title.clone())
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("SomaFM Channels"))
                .highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Yellow))
                .highlight_symbol(">> ");
            
            f.render_stateful_widget(list, top_chunks[0], &mut app.radio_state);

            // Right Panel: Info
            let info_text = if let Some(i) = app.radio_state.selected() {
                if let Some(channel) = app.radio_stations.get(i) {
                    format!(
                        "Title: {}\nDJ: {}\nGenre: {}\nListeners: {}\n\n{}",
                        channel.title,
                        channel.dj,
                        channel.genre,
                        channel.listeners,
                        channel.description
                    )
                } else {
                    "No selection".to_string()
                }
            } else {
                "No selection".to_string()
            };
            
            let info_block = Block::default().borders(Borders::ALL).title("Channel Info");
            let info_paragraph = Paragraph::new(info_text).block(info_block).wrap(ratatui::widgets::Wrap { trim: true });
            f.render_widget(info_paragraph, top_chunks[1]);
        }
    }

    // Help Bar
    let help_text = " q:Quit | TAB:Switch Mode | h:About | j/k/↓/↑:Nav | Enter:Play | Bksp:Up | Space:Pause | +/-:Vol | ←/→:Track ";
    let help_paragraph = Paragraph::new(help_text)
        .style(Style::default().fg(Color::Black).bg(Color::White));
    f.render_widget(help_paragraph, chunks[2]);

    if app.show_about {
        draw_about_modal(f);
    }
}

fn draw_about_modal(f: &mut Frame) {
    let area = centered_rect(60, 25, f.area());
    let block = Block::default().title("About").borders(Borders::ALL);
    let text = [
        "",
        "Cohors - A Terminal Music Player",
        "",
        "Repo: https://github.com/EddieDover/cohors",
        "Author: Eddie Dover <ed@eddiedover.dev>",
        "",
        "Press 'h' or 'Esc' to close",
    ].join("\n");
    
    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);
        
    f.render_widget(Clear, area); // Clear background
    f.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use crate::app::{App, AppMode};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    #[test]
    fn test_ui_draw_file_list() {
        let backend = TestBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("subdir");
        let file = temp.path().join("song.mp3");
        std::fs::create_dir(&dir).unwrap();
        std::fs::File::create(&file).unwrap();
        
        app.items = vec![dir.clone(), file.clone()];
        app.state.select(Some(0));
        
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        
        app.state.select(Some(1));
        app.current_track = Some(file.clone());
        
        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn test_ui_draw_playing_no_duration() {
        let backend = TestBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.current_track = Some(PathBuf::from("stream.mp3"));
        app.track_duration = None;
        
        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn test_ui_draw_about() {
        let backend = TestBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.show_about = true;

        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn test_ui_draw() {
        let backend = TestBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();

        // Just ensure it doesn't panic
        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn test_ui_draw_error() {
        let backend = TestBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.last_error = Some("Test Error".to_string());

        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn test_ui_draw_playing() {
        let backend = TestBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.current_track = Some(PathBuf::from("test.mp3"));
        app.is_paused = false;
        app.track_duration = Some(Duration::from_secs(100));
        app.playback_elapsed = Duration::from_secs(10);
        app.playback_start = Some(Instant::now());

        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn test_ui_draw_radio() {
        let backend = TestBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.mode = AppMode::Radio;

        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }
}
