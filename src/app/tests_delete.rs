use super::*;
use crate::config::{
    AppConfig, IndividualStationConfig, RadioConfig, RadioSourceConfig, StationMapping,
};
use crate::radio::RadioStation;
use crossterm::event::KeyCode;
use std::fs;

use crate::test_utils::with_xdg_config_home;

#[test]
fn test_open_delete_modal_station() {
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

        // Select the station (Group 0 is Custom Stations, Station 0)
        app.radio_groups = vec![crate::radio::RadioGroup {
            title: "Custom Stations".to_string(),
            stations: vec![RadioStation {
                name: "Test Station".to_string(),
                url: "http://test.com".to_string(),
                description: None,
                homepage: None,
                tags: None,
                last_playing: None,
            }],
            is_expanded: true,
        }];
        app.filtered_radio_groups = app.radio_groups.clone();
        app.radio_state.select(Some(1)); // 0 is header, 1 is station
        app.mode = AppMode::Radio;

        // Open delete modal
        app.open_delete_modal();

        // Check if modal is open with correct context
        if let Some(AddModalState::Confirmation { context, .. }) = &app.add_modal_state {
            match context {
                ConfirmationContext::DeleteStation(url) => {
                    assert_eq!(url, "http://test.com");
                }
                _ => panic!("Expected DeleteStation context"),
            }
        } else {
            panic!("Expected Confirmation state");
        }
    });
}

#[test]
fn test_open_delete_modal_source() {
    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    with_xdg_config_home(&config_dir, || {
        let mut app = App::new_test();

        // Create a config with one source
        let radio_config = RadioConfig {
            individual_stations: vec![],
            sources: vec![RadioSourceConfig {
                title: "Test Source".to_string(),
                json_url: "http://source.com".to_string(),
                container: None,
                mapping: StationMapping {
                    station_name: "name".to_string(),
                    station_url: "url".to_string(),
                    description: None,
                    homepage: None,
                    tags: None,
                    last_playing: None,
                },
            }],
        };
        let app_config = AppConfig {
            volume: None,
            subsonic: None,
            radio: radio_config,
            favorites: Default::default(),
            ..Default::default()
        };
        app_config.save_to(&config_path).unwrap();

        // Select the source (Group 0 is Test Source)
        app.radio_groups = vec![crate::radio::RadioGroup {
            title: "Test Source".to_string(),
            stations: vec![],
            is_expanded: false,
        }];
        app.filtered_radio_groups = app.radio_groups.clone();
        app.radio_state.select(Some(0)); // 0 is header
        app.mode = AppMode::Radio;

        // Open delete modal
        app.open_delete_modal();

        // Check if modal is open with correct context
        if let Some(AddModalState::Confirmation { context, .. }) = &app.add_modal_state {
            match context {
                ConfirmationContext::DeleteSource(title) => {
                    assert_eq!(title, "Test Source");
                }
                _ => panic!("Expected DeleteSource context"),
            }
        } else {
            panic!("Expected Confirmation state");
        }
    });
}

#[test]
fn test_confirm_delete_station() {
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

        // Setup app state
        app.add_modal_state = Some(AddModalState::Confirmation {
            message: "Delete?".to_string(),
            context: ConfirmationContext::DeleteStation("http://test.com".to_string()),
        });

        // Confirm delete
        app.handle_add_modal_input(KeyCode::Char('y'));

        // Check if modal closed
        assert!(app.add_modal_state.is_none());

        // Check if deleted from config
        let content = fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();
        assert_eq!(config.radio.individual_stations.len(), 0);
    });
}

#[test]
fn test_cancel_delete() {
    let mut app = App::new_test();

    // Setup app state
    app.add_modal_state = Some(AddModalState::Confirmation {
        message: "Delete?".to_string(),
        context: ConfirmationContext::DeleteStation("http://test.com".to_string()),
    });

    // Cancel delete
    app.handle_add_modal_input(KeyCode::Char('n'));

    // Check if modal closed
    assert!(app.add_modal_state.is_none());
}

#[test]
fn test_open_delete_modal_subsonic() {
    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    with_xdg_config_home(&config_dir, || {
        let mut app = App::new_test();

        // Create a config with one subsonic server
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

        // Open delete modal
        app.open_delete_modal();

        // Check if modal is open with correct context
        if let Some(AddModalState::Confirmation { context, .. }) = &app.add_modal_state {
            match context {
                ConfirmationContext::DeleteSubsonic(server_url) => {
                    assert_eq!(server_url, "http://navi.com");
                }
                _ => panic!("Expected DeleteSubsonic context"),
            }
        } else {
            panic!("Expected Confirmation state");
        }
    });
}

#[test]
fn test_confirm_delete_subsonic() {
    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().to_path_buf();
    let config_path = config_dir.join("cohors/config.json");

    with_xdg_config_home(&config_dir, || {
        let mut app = App::new_test();

        // Create a config with one subsonic server
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

        // Setup app state
        app.add_modal_state = Some(AddModalState::Confirmation {
            message: "Delete?".to_string(),
            context: ConfirmationContext::DeleteSubsonic("http://navi.com".to_string()),
        });

        // Confirm delete
        app.handle_add_modal_input(KeyCode::Char('y'));

        // Check if modal closed
        assert!(app.add_modal_state.is_none());

        // Check if deleted from config
        let content = fs::read_to_string(&config_path).unwrap();
        let config: AppConfig = serde_json::from_str(&content).unwrap();
        assert!(config.subsonic.is_some());
        assert_eq!(config.subsonic.unwrap().sources.len(), 0);
    });
}
