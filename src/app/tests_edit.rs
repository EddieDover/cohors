use super::*;
use crate::config::{
    AppConfig, IndividualStationConfig, RadioConfig, RadioSourceConfig, StationMapping,
};
use crate::radio::RadioStation;
use crossterm::event::KeyCode;
use std::fs;

use crate::test_utils::with_xdg_config_home;

#[test]
fn test_open_edit_modal_station() {
    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    with_xdg_config_home(&config_dir, || {
        let mut app = App::new_test();

        // Create a config with one station
        let radio_config = RadioConfig {
            individual_stations: vec![IndividualStationConfig {
                name: "Test Station".to_string(),
                station_url: "http://test.com".to_string(),
                description: Some("Desc".to_string()),
                homepage: Some("Home".to_string()),
                tags: Some("Tags".to_string()),
            }],
            sources: vec![],
        };
        let app_config = AppConfig {
            volume: None,
            subsonic: None,
            radio: radio_config,
            favorites: Default::default(),
            ..Default::default()
        };
        app_config.save_to(&config_path).unwrap();

        // Select the station (Group 0 is Custom Stations, Station 0)
        app.radio_groups = vec![crate::radio::RadioGroup {
            title: "Custom Stations".to_string(),
            stations: vec![RadioStation {
                name: "Test Station".to_string(),
                url: "http://test.com".to_string(),
                description: Some("Desc".to_string()),
                homepage: Some("Home".to_string()),
                tags: Some("Tags".to_string()),
                last_playing: None,
            }],
            is_expanded: true,
        }];
        app.filtered_radio_groups = app.radio_groups.clone();
        app.radio_state.select(Some(1)); // 0 is header, 1 is station
        app.mode = AppMode::Radio;

        // Open edit modal
        app.open_edit_modal();

        // Check if pre-filled
        if let Some(AddModalState::InputStation {
            name,
            url,
            description,
            homepage,
            tags,
            original_url,
            ..
        }) = &app.add_modal_state
        {
            assert_eq!(name, "Test Station");
            assert_eq!(url, "http://test.com");
            assert_eq!(description, "Desc");
            assert_eq!(homepage, "Home");
            assert_eq!(tags, "Tags");
            assert_eq!(original_url.as_deref(), Some("http://test.com"));
        } else {
            panic!("Expected InputStation state");
        }
    });
}

#[test]
fn test_open_edit_modal_second_station() {
    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    with_xdg_config_home(&config_dir, || {
        let mut app = App::new_test();

        // Create a config with two stations
        let radio_config = RadioConfig {
            individual_stations: vec![
                IndividualStationConfig {
                    name: "Station 1".to_string(),
                    station_url: "http://1.com".to_string(),
                    description: None,
                    homepage: None,
                    tags: None,
                },
                IndividualStationConfig {
                    name: "Station 2".to_string(),
                    station_url: "http://2.com".to_string(),
                    description: None,
                    homepage: None,
                    tags: None,
                },
            ],
            sources: vec![],
        };
        let app_config = AppConfig {
            volume: None,
            subsonic: None,
            radio: radio_config,
            favorites: Default::default(),
            ..Default::default()
        };
        app_config.save_to(&config_path).unwrap();

        // Setup app state
        app.radio_groups = vec![crate::radio::RadioGroup {
            title: "Custom Stations".to_string(),
            stations: vec![
                RadioStation {
                    name: "Station 1".to_string(),
                    url: "http://1.com".to_string(),
                    description: None,
                    homepage: None,
                    tags: None,
                    last_playing: None,
                },
                RadioStation {
                    name: "Station 2".to_string(),
                    url: "http://2.com".to_string(),
                    description: None,
                    homepage: None,
                    tags: None,
                    last_playing: None,
                },
            ],
            is_expanded: true,
        }];
        app.filtered_radio_groups = app.radio_groups.clone();

        // Select second station:
        // 0: Header
        // 1: Station 1
        // 2: Station 2
        app.radio_state.select(Some(2));
        app.mode = AppMode::Radio;

        // Open edit modal
        app.open_edit_modal();

        // Check if pre-filled with Station 2
        if let Some(AddModalState::InputStation {
            name,
            url,
            original_url,
            ..
        }) = &app.add_modal_state
        {
            assert_eq!(name, "Station 2");
            assert_eq!(url, "http://2.com");
            assert_eq!(original_url.as_deref(), Some("http://2.com"));
        } else {
            panic!("Expected InputStation state for Station 2");
        }
    });
}

#[test]
fn test_open_edit_modal_source() {
    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    with_xdg_config_home(&config_dir, || {
        let mut app = App::new_test();

        // Create a config with one source
        let source = RadioSourceConfig {
            title: "Test Source".to_string(),
            json_url: "http://json.com".to_string(),
            container: Some("data".to_string()),
            mapping: StationMapping {
                station_name: "n".to_string(),
                station_url: "u".to_string(),
                description: None,
                homepage: None,
                tags: None,
                last_playing: None,
            },
        };
        let radio_config = RadioConfig {
            individual_stations: vec![],
            sources: vec![source.clone()],
        };
        let app_config = AppConfig {
            volume: None,
            subsonic: None,
            radio: radio_config,
            favorites: Default::default(),
            ..Default::default()
        };
        app_config.save_to(&config_path).unwrap();

        // Select the source (Group 0 is the source)
        app.radio_groups = vec![crate::radio::RadioGroup {
            title: source.title.clone(),
            stations: vec![],
            is_expanded: false,
        }];
        app.filtered_radio_groups = app.radio_groups.clone();
        app.radio_state.select(Some(0)); // 0 is header (source)
        app.mode = AppMode::Radio;

        // Open edit modal
        app.open_edit_modal();

        // Check if pre-filled
        if let Some(AddModalState::InputSource {
            title,
            json_url,
            container,
            map_name,
            map_url,
            original_title,
            ..
        }) = &app.add_modal_state
        {
            assert_eq!(title, "Test Source");
            assert_eq!(json_url, "http://json.com");
            assert_eq!(container, "data");
            assert_eq!(map_name, "n");
            assert_eq!(map_url, "u");
            assert_eq!(original_title.as_deref(), Some("Test Source"));
        } else {
            panic!("Expected InputSource state");
        }
    });
}

#[test]
fn test_edit_station_flow() {
    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    with_xdg_config_home(&config_dir, || {
        let mut app = App::new_test();

        // Create initial config
        let radio_config = RadioConfig {
            individual_stations: vec![IndividualStationConfig {
                name: "Old Name".to_string(),
                station_url: "http://old.com".to_string(),
                description: None,
                homepage: None,
                tags: None,
            }],
            sources: vec![],
        };
        let app_config = AppConfig {
            volume: None,
            subsonic: None,
            radio: radio_config,
            favorites: Default::default(),
            ..Default::default()
        };
        app_config.save_to(&config_path).unwrap();

        // Manually set state to editing
        app.add_modal_state = Some(AddModalState::InputStation {
            name: "New Name".to_string(),
            url: "http://new.com".to_string(),
            description: "".to_string(),
            homepage: "".to_string(),
            tags: "".to_string(),
            focused_field: 0,
            original_url: Some("http://old.com".to_string()),
        });

        // Save
        app.handle_add_modal_input(KeyCode::Enter);

        // Check config
        let content = fs::read_to_string(&config_path).unwrap();
        let new_config: AppConfig = serde_json::from_str(&content).unwrap();
        assert_eq!(new_config.radio.individual_stations.len(), 1);
        assert_eq!(new_config.radio.individual_stations[0].name, "New Name");
        assert_eq!(
            new_config.radio.individual_stations[0].station_url,
            "http://new.com"
        );
    });
}

#[test]
fn test_edit_source_flow() {
    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    with_xdg_config_home(&config_dir, || {
        let mut app = App::new_test();

        // Create initial config
        let source = RadioSourceConfig {
            title: "Old Title".to_string(),
            json_url: "http://old.com".to_string(),
            container: None,
            mapping: StationMapping {
                station_name: "n".to_string(),
                station_url: "u".to_string(),
                description: None,
                homepage: None,
                tags: None,
                last_playing: None,
            },
        };
        let radio_config = RadioConfig {
            individual_stations: vec![],
            sources: vec![source],
        };
        let app_config = AppConfig {
            volume: None,
            subsonic: None,
            radio: radio_config,
            favorites: Default::default(),
            ..Default::default()
        };
        app_config.save_to(&config_path).unwrap();

        // Manually set state to editing
        app.add_modal_state = Some(AddModalState::InputSource {
            title: "New Title".to_string(),
            json_url: "http://new.com".to_string(),
            container: "".to_string(),
            map_name: "n".to_string(),
            map_url: "u".to_string(),
            map_desc: "".to_string(),
            map_home: "".to_string(),
            map_tags: "".to_string(),
            focused_field: 0,
            original_title: Some("Old Title".to_string()),
        });

        // Save
        app.handle_add_modal_input(KeyCode::Enter);

        // Check config
        let content = fs::read_to_string(&config_path).unwrap();
        let new_config: AppConfig = serde_json::from_str(&content).unwrap();
        assert_eq!(new_config.radio.sources.len(), 1);
        assert_eq!(new_config.radio.sources[0].title, "New Title");
        assert_eq!(new_config.radio.sources[0].json_url, "http://new.com");
    });
}

#[test]
fn test_reload_stations_integration() {
    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    with_xdg_config_home(&config_dir, || {
        let mut app = App::new_test();

        // Create a dummy config so reload doesn't fail immediately (though it might fail on fetch, but receiver should be set)
        let radio_config = RadioConfig {
            individual_stations: vec![],
            sources: vec![],
        };
        let app_config = AppConfig {
            volume: None,
            subsonic: None,
            radio: radio_config,
            favorites: Default::default(),
            ..Default::default()
        };
        app_config.save_to(&config_path).unwrap();

        // Trigger reload
        app.reload_stations();

        // Check if receiver is set
        assert!(app.station_receiver.is_some());

        // Since reload_stations spawns a thread that does real IO or fails,
        // we might just check that the receiver exists.
        // Manually inject a receiver to test on_tick handling.
        let (tx, rx) = std::sync::mpsc::channel();
        app.station_receiver = Some(rx);

        let groups = vec![crate::radio::RadioGroup {
            title: "Test".to_string(),
            stations: vec![],
            is_expanded: false,
        }];
        tx.send(Ok(groups.clone())).unwrap();

        app.on_tick();

        assert!(app.station_receiver.is_none());
        assert_eq!(app.radio_groups.len(), 1);
        assert_eq!(app.radio_groups[0].title, "Test");
        assert!(app.notification.is_some());
    });
}

#[test]
fn test_open_edit_modal_subsonic() {
    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    with_xdg_config_home(&config_dir, || {
        let mut app = App::new_test();

        let app_config = AppConfig {
            volume: None,
            subsonic: Some(crate::config::SubsonicConfig {
                sources: vec![crate::config::SubsonicSourceConfig {
                    server_url: "http://navi.com".to_string(),
                    username: "user".to_string(),
                    password: Some("pass".to_string()),
                    auth_token: None,
                }],
            }),
            radio: Default::default(),
            favorites: Default::default(),
            ..Default::default()
        };
        app_config.save_to(&config_path).unwrap();

        // Load into app
        app.subsonic_clients = app_config
            .subsonic
            .unwrap()
            .sources
            .into_iter()
            .map(crate::subsonic::SubsonicClient::new)
            .collect();
        app.subsonic_view = SubsonicView::Servers;
        app.subsonic_state.select(Some(0));
        app.mode = AppMode::Subsonic;

        // Open edit modal
        app.open_edit_modal();

        // Check if modal is open with correct input fields
        if let Some(AddModalState::InputSubsonic {
            server_url,
            username,
            password,
            focused_field,
            original_url,
        }) = &app.add_modal_state
        {
            assert_eq!(server_url, "http://navi.com");
            assert_eq!(username, "user");
            assert_eq!(password, "pass");
            assert_eq!(*focused_field, 0);
            assert_eq!(original_url.as_ref().unwrap(), "http://navi.com");
        } else {
            panic!("Expected InputSubsonic state");
        }

        // Change url a bit
        app.handle_add_modal_input(KeyCode::Char('2'));

        // Save
        app.handle_add_modal_input(KeyCode::Enter);

        // Check if saved and mode reset
        assert!(app.add_modal_state.is_none());
        assert!(
            app.notification
                .clone()
                .unwrap()
                .0
                .contains("Failed to load")
                || app.notification.unwrap().0.contains("Saved")
        );

        // Verify config updated
        let saved_config = AppConfig::load().unwrap();
        assert_eq!(
            saved_config.subsonic.unwrap().sources[0].server_url,
            "http://navi.com2"
        );
    });
}
