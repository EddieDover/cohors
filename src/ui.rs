use crate::app::{AddModalState, App, AppMode, LoopMode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        BarChart, Block, Borders, Cell, Clear, Gauge, List, ListItem, ListState, Paragraph, Row,
        Table,
    },
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
    } else if app.source_receiver.is_some() {
        ("Loading...".to_string(), 0.0, "Loading...".to_string())
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
                .filtered_items
                .iter()
                .map(|path| {
                    let mut name = if path.ends_with("..") {
                        "..".to_string()
                    } else {
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string()
                    };

                    if app.favorites.is_favorite_file(path) {
                        name.push_str(" ★");
                    }

                    let style = if path.is_dir() {
                        if name != ".." {
                            name.push('/');
                        }
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
                if let Some(path) = app.filtered_items.get(i) {
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

            'outer: for group in &app.filtered_radio_groups {
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
                            let mut name = format!("  {}", station.name);
                            if app.favorites.is_favorite_station(station) {
                                name.push_str(" ★");
                            }
                            items.push(ListItem::new(name));
                        }

                        if selected_opt == Some(current_idx) {
                            selected_item_info = Some((
                                format!(
                                    "Name: {}\nDescription:{}\nTags: {}\nLast Playing: {}\nHomepage: {}\nStation URL: {}",
                                    station.name,
                                    station.description.as_deref().unwrap_or(""),
                                    station.tags.as_deref().unwrap_or(""),
                                    station.last_playing.as_deref().unwrap_or(""),
                                    station.homepage.as_deref().unwrap_or(""),
                                    station.url
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

            let info_paragraph = Paragraph::new(full_text)
                .block(info_block)
                .wrap(ratatui::widgets::Wrap { trim: true });
            f.render_widget(info_paragraph, top_chunks[1]);
        }
        AppMode::Favorites => {
            let top_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
                .split(chunks[0]);

            let mut items = Vec::new();
            // Files
            for path in &app.favorites.files {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let style = if path.is_dir() {
                    Style::default().fg(Color::Blue)
                } else {
                    Style::default()
                };
                items.push(ListItem::new(format!("{} ★", name)).style(style));
            }
            // Stations
            for station in &app.favorites.stations {
                let name = format!("{} ★", station.name);
                items.push(ListItem::new(name));
            }

            let list = List::new(items)
                .block(Block::default().borders(Borders::ALL).title("Favorites"))
                .highlight_style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .fg(Color::Yellow),
                )
                .highlight_symbol(">> ");

            f.render_stateful_widget(list, top_chunks[0], &mut app.favorites_state);

            // Info
            let info_text = if let Some(i) = app.favorites_state.selected() {
                if i < app.favorites.files.len() {
                    if let Some(path) = app.favorites.files.get(i) {
                        format!("File: {}", path.display())
                    } else {
                        String::new()
                    }
                } else {
                    let station_idx = i - app.favorites.files.len();
                    if let Some(station) = app.favorites.stations.get(station_idx) {
                        format!("Station: {}\nURL: {}", station.name, station.url)
                    } else {
                        String::new()
                    }
                }
            } else {
                "No selection".to_string()
            };
            let info_block = Block::default().borders(Borders::ALL).title("Info");
            let info_paragraph = Paragraph::new(info_text).block(info_block);
            f.render_widget(info_paragraph, top_chunks[1]);
        }
    }

    // Help Bar
    let version_text = if let Some(latest) = &app.latest_version {
        format!("{} -> {} Update Available ", app.current_version, latest)
    } else {
        format!("{} ", app.current_version)
    };

    let version_width = version_text.len() as u16;

    let help_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(version_width)])
        .split(chunks[2]);

    if app.is_searching {
        let search_text = format!("Search: {}_", app.search_query);
        let search_paragraph =
            Paragraph::new(search_text).style(Style::default().fg(Color::Black).bg(Color::Yellow));
        f.render_widget(search_paragraph, help_layout[0]);
    } else if !app.search_query.is_empty() {
        let search_text = format!("Filter: {}", app.search_query);
        let search_paragraph =
            Paragraph::new(search_text).style(Style::default().fg(Color::Black).bg(Color::Blue));
        f.render_widget(search_paragraph, help_layout[0]);
    } else {
        let help_text = " ?:Help | q:Quit | TAB:Switch Mode ";
        let help_paragraph =
            Paragraph::new(help_text).style(Style::default().fg(Color::Black).bg(Color::White));
        f.render_widget(help_paragraph, help_layout[0]);
    }

    let version_paragraph = Paragraph::new(version_text)
        .style(Style::default().fg(Color::Black).bg(Color::White))
        .alignment(ratatui::layout::Alignment::Right);
    f.render_widget(version_paragraph, help_layout[1]);

    draw_notification(f, app);

    if app.show_help {
        draw_help_modal(f);
    }

    if app.add_modal_state.is_some() {
        draw_add_modal(f, app);
    }
}

fn draw_add_modal(f: &mut Frame, app: &App) {
    if let Some(state) = &app.add_modal_state {
        let area = centered_rect(60, 85, f.area());
        f.render_widget(Clear, area);
        let block = Block::default().title("Add New").borders(Borders::ALL);
        f.render_widget(block.clone(), area);
        let inner = block.inner(area);

        match state {
            AddModalState::Selection => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(3), Constraint::Length(3)].as_ref())
                    .margin(2)
                    .split(inner);

                let p1 = Paragraph::new("Press 's' to add a Station")
                    .alignment(ratatui::layout::Alignment::Center);
                let p2 = Paragraph::new("Press 'r' to add a Radio Source")
                    .alignment(ratatui::layout::Alignment::Center);

                f.render_widget(p1, chunks[0]);
                f.render_widget(p2, chunks[1]);
            }
            AddModalState::InputStation {
                name,
                url,
                description,
                homepage,
                tags,
                focused_field,
                ..
            } => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(
                        [
                            Constraint::Length(3), // Name
                            Constraint::Length(3), // URL
                            Constraint::Length(3), // Desc
                            Constraint::Length(3), // Home
                            Constraint::Length(3), // Tags
                            Constraint::Min(0),
                        ]
                        .as_ref(),
                    )
                    .margin(1)
                    .split(inner);

                let fields = [
                    ("Name", name),
                    ("URL", url),
                    ("Description", description),
                    ("Homepage", homepage),
                    ("Tags", tags),
                ];

                for (i, (label, value)) in fields.iter().enumerate() {
                    let style = if *focused_field == i {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    };
                    let p = Paragraph::new(value.as_str()).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(*label)
                            .style(style),
                    );
                    f.render_widget(p, chunks[i]);
                }

                let help = Paragraph::new("Enter: Save | Esc: Cancel | Tab: Next Field")
                    .alignment(ratatui::layout::Alignment::Center)
                    .style(Style::default().fg(Color::Gray));
                f.render_widget(help, chunks[5]);
            }
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
                ..
            } => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints(
                        [
                            Constraint::Length(3), // Title
                            Constraint::Length(3), // JSON URL
                            Constraint::Length(3), // Container
                            Constraint::Length(3), // Mapping Row 1
                            Constraint::Length(3), // Mapping Row 2
                            Constraint::Length(3), // Mapping Row 3
                            Constraint::Min(0),    // Help
                        ]
                        .as_ref(),
                    )
                    .margin(1)
                    .split(inner);

                // Row 1: Title
                let style = if *focused_field == 0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                f.render_widget(
                    Paragraph::new(title.as_str()).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Title")
                            .style(style),
                    ),
                    chunks[0],
                );

                // Row 2: JSON URL
                let style = if *focused_field == 1 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                f.render_widget(
                    Paragraph::new(json_url.as_str()).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("JSON URL")
                            .style(style),
                    ),
                    chunks[1],
                );

                // Row 3: Container
                let style = if *focused_field == 2 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                f.render_widget(
                    Paragraph::new(container.as_str()).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Container (Optional)")
                            .style(style),
                    ),
                    chunks[2],
                );

                // Row 4: Map Name | Map URL
                let row4 = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(chunks[3]);

                let style = if *focused_field == 3 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                f.render_widget(
                    Paragraph::new(map_name.as_str()).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Map: Name")
                            .style(style),
                    ),
                    row4[0],
                );

                let style = if *focused_field == 4 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                f.render_widget(
                    Paragraph::new(map_url.as_str()).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Map: URL")
                            .style(style),
                    ),
                    row4[1],
                );

                // Row 5: Map Desc | Map Home
                let row5 = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(chunks[4]);

                let style = if *focused_field == 5 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                f.render_widget(
                    Paragraph::new(map_desc.as_str()).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Map: Desc")
                            .style(style),
                    ),
                    row5[0],
                );

                let style = if *focused_field == 6 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                f.render_widget(
                    Paragraph::new(map_home.as_str()).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Map: Home")
                            .style(style),
                    ),
                    row5[1],
                );

                // Row 6: Map Tags
                let style = if *focused_field == 7 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                };
                f.render_widget(
                    Paragraph::new(map_tags.as_str()).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Map: Tags")
                            .style(style),
                    ),
                    chunks[5],
                );

                // Help
                let help = Paragraph::new("Enter: Save | Esc: Cancel | Tab: Next Field")
                    .alignment(ratatui::layout::Alignment::Center)
                    .style(Style::default().fg(Color::Gray));
                f.render_widget(help, chunks[6]);
            }
        }
    }
}

fn draw_help_modal(f: &mut Frame) {
    let area = centered_rect(80, 90, f.area());
    f.render_widget(Clear, area); // Clear background

    let block = Block::default().title("Help").borders(Borders::ALL);
    f.render_widget(block.clone(), area);

    let inner_area = block.inner(area);

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(7)].as_ref())
        .margin(1)
        .split(inner_area);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(vertical_chunks[0]);

    let rows_left = vec![
        // General
        Row::new(vec![Cell::from("General"), Cell::from("")]).style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        ),
        Row::new(vec![Cell::from("?"), Cell::from("Toggle Help")]),
        Row::new(vec![Cell::from("q"), Cell::from("Quit")]),
        Row::new(vec![Cell::from("TAB"), Cell::from("Switch Mode")]),
        Row::new(vec![Cell::from("/"), Cell::from("Search")]),
        // Playback
        Row::new(vec![Cell::from("")]),
        Row::new(vec![Cell::from("Playback"), Cell::from("")]).style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        ),
        Row::new(vec![Cell::from("Space"), Cell::from("Play/Pause")]),
        Row::new(vec![Cell::from("+/-"), Cell::from("Volume Up/Down")]),
        Row::new(vec![Cell::from("l"), Cell::from("Toggle Loop")]),
        Row::new(vec![Cell::from("← / →"), Cell::from("Prev / Next Track")]),
    ];

    let table_left = Table::new(
        rows_left,
        [Constraint::Percentage(30), Constraint::Percentage(70)],
    )
    .header(
        Row::new(vec!["Key", "Action"])
            .style(Style::default().fg(Color::Yellow))
            .bottom_margin(1),
    )
    .column_spacing(1);

    f.render_widget(table_left, chunks[0]);

    let rows_right = vec![
        // Navigation
        Row::new(vec![Cell::from("Navigation"), Cell::from("")]).style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        ),
        Row::new(vec![Cell::from("j / ↓"), Cell::from("Down")]),
        Row::new(vec![Cell::from("k / ↑"), Cell::from("Up")]),
        Row::new(vec![Cell::from("Enter"), Cell::from("Play / Enter Dir")]),
        Row::new(vec![Cell::from("Bksp"), Cell::from("Go Up Dir")]),
        // Files
        Row::new(vec![Cell::from("")]),
        Row::new(vec![Cell::from("Files / Radio"), Cell::from("")]).style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        ),
        Row::new(vec![Cell::from("h"), Cell::from("Toggle Hidden")]),
        Row::new(vec![Cell::from("f"), Cell::from("Toggle Favorite")]),
        Row::new(vec![Cell::from("x"), Cell::from("Export Station")]),
        Row::new(vec![Cell::from("a"), Cell::from("Add Station/Source")]),
    ];

    let table_right = Table::new(
        rows_right,
        [Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .header(
        Row::new(vec!["Key", "Action"])
            .style(Style::default().fg(Color::Yellow))
            .bottom_margin(1),
    )
    .column_spacing(1);

    f.render_widget(table_right, chunks[1]);

    let rows_about = vec![
        // About
        Row::new(vec![Cell::from("")]),
        Row::new(vec![Cell::from("About"), Cell::from("")]).style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        ),
        Row::new(vec![
            Cell::from("Version"),
            Cell::from(env!("CARGO_PKG_VERSION")),
        ]),
        Row::new(vec![
            Cell::from("Author"),
            Cell::from(env!("CARGO_PKG_AUTHORS")),
        ]),
        Row::new(vec![
            Cell::from("Repo"),
            Cell::from(env!("CARGO_PKG_REPOSITORY")),
        ]),
    ];

    let table_about = Table::new(
        rows_about,
        [Constraint::Percentage(15), Constraint::Percentage(85)],
    )
    .column_spacing(1);

    f.render_widget(table_about, vertical_chunks[1]);
}

fn draw_notification(f: &mut Frame, app: &App) {
    if let Some((msg, _)) = &app.notification {
        let size = f.area();
        let width = (msg.len() as u16 + 4).clamp(20, 40);
        let height = 3;
        let area = Rect::new(size.width.saturating_sub(width + 2), 1, width, height);
        f.render_widget(Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Green));
        let paragraph = Paragraph::new(msg.as_str())
            .block(block)
            .wrap(ratatui::widgets::Wrap { trim: true })
            .alignment(ratatui::layout::Alignment::Center);
        f.render_widget(paragraph, area);
    }
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
mod tests;
