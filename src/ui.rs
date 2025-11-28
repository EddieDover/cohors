use crate::app::{App, AppMode, LoopMode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{BarChart, Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph},
};
use std::time::Duration;

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Vertical split: Top (Main), Status (4), Help (1)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Min(0),
                Constraint::Length(4),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
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
            format!(
                "{:02}:{:02} / {:02}:{:02}",
                elapsed_secs / 60,
                elapsed_secs % 60,
                total_secs / 60,
                total_secs % 60
            )
        } else {
            format!("{:02}:{:02}", elapsed_secs / 60, elapsed_secs % 60)
        };

        let loop_status = match app.loop_mode {
            LoopMode::Off => "Loop: Off",
            LoopMode::Track => "Loop: Track",
            LoopMode::All => "Loop: All",
        };

        (
            format!(
                "{}: {} (Vol: {:.0}%) [{}]",
                state,
                name,
                app.volume * 100.0,
                loop_status
            ),
            ratio,
            time_str,
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
            ("Sub", 0),
            ("Bass", 0),
            ("LowM", 0),
            ("Mid", 0),
            ("HighM", 0),
            ("Pres", 0),
            ("Bril", 0),
            ("Air", 0),
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
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(name).style(style)
                })
                .collect();

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Files"))
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Yellow),
                )
                .highlight_symbol(">> ");

            f.render_stateful_widget(list, top_chunks[0], &mut app.state);

            // Right Panel: Info
            let info_text = if let Some(i) = app.state.selected() {
                if let Some(path) = app.items.get(i) {
                    format!(
                        "Selected: {}\nPath: {}",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        path.display()
                    )
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

            let list_area = top_chunks[0];
            let list_height = list_area.height as usize;

            // Calculate visible window
            let selected_opt = app.radio_state.selected();
            let selected = selected_opt.unwrap_or(0);
            let offset = app.radio_state.offset();

            let new_offset = if selected < offset {
                selected
            } else if selected >= offset + list_height {
                selected.saturating_sub(list_height).saturating_add(1)
            } else {
                offset
            };

            if new_offset != offset {
                app.radio_state = app.radio_state.clone().with_offset(new_offset);
            }

            let start = new_offset;
            let end = start + list_height;

            let mut items = Vec::new();
            let mut current_idx = 0;
            let mut selected_item_info = None;

            'outer: for group in &app.radio_groups {
                // Group Header
                if current_idx >= start && current_idx < end {
                    let prefix = if group.is_expanded { "v " } else { "> " };
                    let title = format!("{}{}", prefix, group.title);
                    let style = Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD);
                    items.push(ListItem::new(title).style(style));
                }

                if selected_opt == Some(current_idx) {
                    selected_item_info = Some((
                        format!("Group: {} ({} stations)", group.title, group.stations.len()),
                        String::new(),
                    ));
                }
                current_idx += 1;

                if group.is_expanded {
                    let station_count = group.stations.len();

                    // Optimization: Skip if entire group is before start
                    if current_idx + station_count <= start {
                        current_idx += station_count;
                        continue;
                    }

                    for station in &group.stations {
                        if current_idx >= end {
                            // If we found the selected item, we can break
                            if selected_item_info.is_some() {
                                break 'outer;
                            }
                            // If selected is further down (shouldn't happen with correct offset),
                            // we might miss info, but we prioritize rendering speed.
                            break 'outer;
                        }

                        if current_idx >= start {
                            let name = format!("  {}", station.name);
                            items.push(ListItem::new(name));
                        }

                        if selected_opt == Some(current_idx) {
                            selected_item_info = Some((
                                format!(
                                    "Name: {}\nTags: {}\nLast Playing: {}\nHomepage: {}\nImage: {}",
                                    station.name,
                                    station.tags.as_deref().unwrap_or(""),
                                    station.last_playing.as_deref().unwrap_or(""),
                                    station.homepage.as_deref().unwrap_or(""),
                                    station.image.as_deref().unwrap_or("")
                                ),
                                station.description.as_deref().unwrap_or("").to_string(),
                            ));
                        }
                        current_idx += 1;
                    }
                }
            }

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Radio Stations"),
                )
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Yellow),
                )
                .highlight_symbol(">> ");

            // Use a temporary state for rendering the windowed list
            let relative_selected = selected_opt.map(|s| s.saturating_sub(start));
            let mut render_state = ListState::default()
                .with_selected(relative_selected)
                .with_offset(0);

            f.render_stateful_widget(list, top_chunks[0], &mut render_state);

            // Right Panel: Info
            f.render_widget(Clear, top_chunks[1]);
            let (info_text, description) =
                selected_item_info.unwrap_or(("No selection".to_string(), String::new()));
            let full_text = if description.is_empty() {
                info_text
            } else {
                format!("{}\n\n{}", info_text, description)
            };

            let info_block = Block::default().borders(Borders::ALL).title("Station Info");

            // Check for image
            let mut image_widget = None;
            let mut image_url_to_load = None;

            if let Some(idx) = selected_opt {
                let mut curr = 0;
                for group in &app.radio_groups {
                    if curr == idx {
                        break;
                    }
                    curr += 1;
                    if group.is_expanded {
                        if idx < curr + group.stations.len() {
                            let station = &group.stations[idx - curr];
                            if let Some(url) = &station.image
                                && !url.is_empty()
                            {
                                image_url_to_load = Some(url.clone());
                            }
                            break;
                        }
                        curr += group.stations.len();
                    }
                }
            }

            if let Some(url) = image_url_to_load {
                if !app.image_cache.contains_key(&url) {
                    app.trigger_image_load(url.clone());
                }

                if let Some(img) = app.image_cache.get(&url)
                    && let Some(picker) = &mut app.picker
                {
                    image_widget = Some((picker.new_resize_protocol(img.clone()), img));
                }
            }

            if let Some((mut protocol, _img)) = image_widget {
                // Split info panel into text and image
                let info_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(10), Constraint::Percentage(50)].as_ref())
                    .split(top_chunks[1]);

                let info_paragraph = Paragraph::new(full_text)
                    .block(info_block)
                    .wrap(ratatui::widgets::Wrap { trim: true });
                f.render_widget(info_paragraph, info_chunks[0]);

                let image_block = Block::default().borders(Borders::ALL).title("Cover");
                let image_area = image_block.inner(info_chunks[1]);
                f.render_widget(image_block, info_chunks[1]);

                f.render_widget(Clear, image_area);

                let image = ratatui_image::StatefulImage::default();
                f.render_stateful_widget(image, image_area, &mut protocol);
            } else {
                let info_paragraph = Paragraph::new(full_text)
                    .block(info_block)
                    .wrap(ratatui::widgets::Wrap { trim: true });
                f.render_widget(info_paragraph, top_chunks[1]);
            }
        }
    }

    // Help Bar
    let help_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(10)])
        .split(chunks[2]);

    let help_text = match app.mode {
        AppMode::FileSystem => {
            " q:Quit | TAB:Switch Mode | ?:About | h:Hidden | j/k/↓/↑:Nav | Enter:Play | Bksp:Up | Space:Pause | +/-:Vol | ←/→:Track | l:Loop "
        }
        AppMode::Radio => {
            " q:Quit | TAB:Switch Mode | ?:About | j/k/↓/↑:Nav | Enter:Play | Space:Pause | +/-:Vol "
        }
    };
    let help_paragraph =
        Paragraph::new(help_text).style(Style::default().fg(Color::Black).bg(Color::White));
    f.render_widget(help_paragraph, help_layout[0]);

    let version_text = format!("v{} ", env!("CARGO_PKG_VERSION"));
    let version_paragraph = Paragraph::new(version_text)
        .style(Style::default().fg(Color::Black).bg(Color::White))
        .alignment(ratatui::layout::Alignment::Right);
    f.render_widget(version_paragraph, help_layout[1]);

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
        "Press '?' or 'Esc' to close",
    ]
    .join("\n");

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
    use crate::app::{App, AppMode};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
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

        app.radio_groups.push(crate::radio::RadioGroup {
            title: "Test Group".to_string(),
            stations: vec![crate::radio::RadioStation {
                name: "Test Station".to_string(),
                url: "http://test.com".to_string(),
                description: None,
                homepage: None,
                tags: None,
                image: None,
                last_playing: None,
            }],
            is_expanded: true,
        });
        app.radio_state.select(Some(0));

        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn test_ui_draw_radio_large_list() {
        let backend = TestBackend::new(100, 20); // Small height to force scrolling
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.mode = AppMode::Radio;

        let mut stations = Vec::new();
        for i in 0..50 {
            stations.push(crate::radio::RadioStation {
                name: format!("Station {}", i),
                url: "http://test.com".to_string(),
                description: Some("Desc".to_string()),
                homepage: None,
                tags: None,
                image: None,
                last_playing: None,
            });
        }

        app.radio_groups.push(crate::radio::RadioGroup {
            title: "Large Group".to_string(),
            stations,
            is_expanded: true,
        });

        // Select an item that requires scrolling (e.g., index 30)
        // Index 0 is group header, stations start at 1.
        // So station 30 is at index 31.
        app.radio_state.select(Some(31));

        terminal.draw(|f| draw(f, &mut app)).unwrap();

        // Verify offset was updated
        assert!(app.radio_state.offset() > 0);
    }

    #[test]
    fn test_ui_draw_radio_with_image() {
        let backend = TestBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.mode = AppMode::Radio;

        // Setup a station with an image URL
        let station_url = "http://example.com/image.png".to_string();
        app.radio_groups.push(crate::radio::RadioGroup {
            title: "Test Group".to_string(),
            stations: vec![crate::radio::RadioStation {
                name: "Test Station".to_string(),
                url: "http://test.com".to_string(),
                description: None,
                homepage: None,
                tags: None,
                image: Some(station_url.clone()),
                last_playing: None,
            }],
            is_expanded: true,
        });
        app.radio_state.select(Some(1)); // Select the station

        // Manually populate the image cache
        let img = image::DynamicImage::new_rgb8(10, 10);
        app.image_cache.insert(station_url.clone(), img);

        // Manually set a picker (using Halfblocks for test environment safety)
        app.picker = Some(ratatui_image::picker::Picker::from_fontsize((8, 16)));

        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn test_ui_draw_radio_scrolling_optimization() {
        let backend = TestBackend::new(100, 10); // Very short height
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.mode = AppMode::Radio;

        // Group 1: 5 stations (Index 0-5)
        let mut stations1 = Vec::new();
        for i in 0..5 {
            stations1.push(crate::radio::RadioStation {
                name: format!("Station 1-{}", i),
                url: "u".to_string(),
                description: None,
                homepage: None,
                tags: None,
                image: None,
                last_playing: None,
            });
        }
        app.radio_groups.push(crate::radio::RadioGroup {
            title: "Group 1".to_string(),
            stations: stations1,
            is_expanded: true,
        });

        // Group 2: 5 stations (Index 6-11)
        let mut stations2 = Vec::new();
        for i in 0..5 {
            stations2.push(crate::radio::RadioStation {
                name: format!("Station 2-{}", i),
                url: "u".to_string(),
                description: None,
                homepage: None,
                tags: None,
                image: None,
                last_playing: None,
            });
        }
        app.radio_groups.push(crate::radio::RadioGroup {
            title: "Group 2".to_string(),
            stations: stations2,
            is_expanded: true,
        });

        // Select something in Group 2 to force scrolling Group 1 out of view
        // List height is ~10 (minus borders/titles/etc).
        // If we select index 10 (Station 2-4), offset should be around 10-height+1.
        app.radio_state.select(Some(10));

        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn test_ui_draw_radio_image_loading() {
        let backend = TestBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.mode = AppMode::Radio;

        let station_url = "http://example.com/image.png".to_string();
        app.radio_groups.push(crate::radio::RadioGroup {
            title: "Test Group".to_string(),
            stations: vec![crate::radio::RadioStation {
                name: "Test Station".to_string(),
                url: "http://test.com".to_string(),
                description: None,
                homepage: None,
                tags: None,
                image: Some(station_url.clone()),
                last_playing: None,
            }],
            is_expanded: true,
        });
        app.radio_state.select(Some(1));

        terminal.draw(|f| draw(f, &mut app)).unwrap();

        assert!(app.image_loading.contains(&station_url));
    }

    #[test]
    fn test_ui_draw_visualizer_empty_data() {
        let backend = TestBackend::new(100, 50);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.current_track = Some(PathBuf::from("test.mp3"));
        app.is_paused = false;
        // Force empty spectrum data
        *app.spectrum_data.lock().unwrap() = Vec::new();
        
        terminal.draw(|f| draw(f, &mut app)).unwrap();
    }

    #[test]
    fn test_ui_draw_radio_scroll_up() {
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new_test();
        app.mode = AppMode::Radio;
        
        // Add enough items
        let mut stations = Vec::new();
        for i in 0..20 {
            stations.push(crate::radio::RadioStation {
                name: format!("Station {}", i),
                url: "u".to_string(),
                description: None,
                homepage: None,
                tags: None,
                image: None,
                last_playing: None,
            });
        }
        app.radio_groups.push(crate::radio::RadioGroup {
            title: "Group".to_string(),
            stations,
            is_expanded: true,
        });
        
        // Set offset to 10, selected to 5
        app.radio_state = app.radio_state.clone().with_offset(10);
        app.radio_state.select(Some(5));
        
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        
        // Offset should become 5
        assert_eq!(app.radio_state.offset(), 5);
    }
}
