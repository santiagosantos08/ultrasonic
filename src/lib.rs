slint::include_modules!();

pub mod api;
pub mod audio;
pub mod config;
pub mod controller;
pub mod models;
pub mod theme;
pub mod webui;

use controller::{attach_callbacks_and_run, AppState};
use std::cell::RefCell;
use std::rc::Rc;

pub fn run_app() -> Result<(), slint::PlatformError> {
    let state = Rc::new(RefCell::new(AppState::new()));

    #[cfg(not(target_arch = "wasm32"))]
    {
        let cfg = state.borrow().config.clone();
        if cfg.webui_enabled {
            webui::spawn_webui_server(cfg.webui_port);
        }
    }

    let ui = AppWindow::new()?;
    attach_callbacks_and_run(&ui, state);
    ui.run()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).expect("Failed to initialize Slint Android backend");
    let _ = run_app();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_main() {
    let _ = run_app();
}

#[cfg(test)]
mod tests {
    use super::api::{parse_lrc_or_plain_lyrics, NavidromeClient};
    use super::config::OfflineIndex;
    use super::models::{Album, PlayQueue, Song};

    #[test]
    fn test_multi_library_query_url_generation() {
        let client = NavidromeClient::new("https://navidrome.example.com", "alice", "secret123");
        let url = client.build_url(
            "getAlbumList2",
            &[
                ("type", "newest".to_string()),
                ("musicFolderId", "lib-1".to_string()),
                ("musicFolderId", "lib-2".to_string()),
            ],
        );
        assert!(url.starts_with("https://navidrome.example.com/rest/getAlbumList2?"));
        assert!(url.contains("u=alice"));
        assert!(url.contains("v=1.16.1&c=Ultrasonic&f=json"));
        assert!(url.contains("musicFolderId=lib-1&musicFolderId=lib-2"));
    }

    #[test]
    fn test_stream_and_download_rate_urls() {
        let client = NavidromeClient::new("http://localhost:4533", "admin", "pass");
        let stream = client.stream_url("song-md5-abc", "opus", 192, 0);
        assert!(stream.contains("/rest/stream?"));
        assert!(stream.contains("id=song-md5-abc"));
        assert!(stream.contains("format=opus"));
        assert!(stream.contains("maxBitRate=192"));

        let dl = client.download_url("album-md5-xyz", "flac", 0);
        assert!(dl.contains("/rest/download?"));
        assert!(dl.contains("id=album-md5-xyz"));
        assert!(dl.contains("format=flac"));
        assert!(!dl.contains("maxBitRate="));
    }

    #[test]
    fn test_navidrome_string_ids_and_playqueue_current() {
        let json_str = r#"{
            "current": "9c8f7b6a-string-id",
            "position": 42000,
            "username": "admin",
            "entry": [
                {
                    "id": "9c8f7b6a-string-id",
                    "title": "Test Track",
                    "bitDepth": 24,
                    "samplingRate": 96000,
                    "suffix": "flac"
                }
            ]
        }"#;
        let pq: PlayQueue = serde_json::from_str(json_str).expect("Should parse Navidrome string current ID");
        assert_eq!(pq.current.as_deref(), Some("9c8f7b6a-string-id"));
        assert_eq!(pq.entry.len(), 1);
        assert_eq!(pq.entry[0].quality_badge(), "FLAC 24/96");

        let legacy_album_json = r#"{"id": 42, "name": "Numeric ID Album", "songCount": 10, "duration": 600}"#;
        let alb: Album = serde_json::from_str(legacy_album_json).expect("Should also accept numeric IDs");
        assert_eq!(alb.id, "42");
    }

    #[test]
    fn test_lrc_synchronized_lyrics_parser() {
        let lrc = "[00:12.50]First line of song\n[01:05.00]Second line of song";
        let parsed = parse_lrc_or_plain_lyrics(lrc);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].start, Some(12_500));
        assert_eq!(parsed[0].value, "First line of song");
        assert_eq!(parsed[1].start, Some(65_000));
        assert_eq!(parsed[1].value, "Second line of song");
    }

    #[test]
    fn test_auto_download_offline_index() {
        let mut idx = OfflineIndex::default();
        let song = Song {
            id: "auto-dl-test-track".to_string(),
            title: "Auto Downloaded Song".to_string(),
            suffix: Some("wav".to_string()),
            ..Default::default()
        };
        let dummy_bytes = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        idx.store_track(song, &dummy_bytes, "raw", 0)
            .expect("Should store track locally");
        assert!(idx.is_downloaded("auto-dl-test-track"));
    }

    #[test]
    fn test_bundled_themes_and_hex_color_parsing() {
        use crate::theme::{get_bundled_themes, parse_hex_color};
        let themes = get_bundled_themes();
        assert!(themes.len() >= 7);
        let ids: Vec<&str> = themes.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"default"));
        assert!(ids.contains(&"emerald"));
        assert!(ids.contains(&"ocean"));
        assert!(ids.contains(&"amethyst"));
        assert!(ids.contains(&"amber"));
        assert!(ids.contains(&"light"));
        assert!(ids.contains(&"nord"));

        let red = parse_hex_color("#fa2d48");
        assert_eq!(red.red(), 250);
        assert_eq!(red.green(), 45);
        assert_eq!(red.blue(), 72);
        assert_eq!(red.alpha(), 255);

        let red_alpha = parse_hex_color("#fa2d4826");
        assert_eq!(red_alpha.red(), 250);
        assert_eq!(red_alpha.green(), 45);
        assert_eq!(red_alpha.blue(), 72);
        assert_eq!(red_alpha.alpha(), 0x26);
    }

    #[test]
    fn test_custom_theme_creation_and_config() {
        use crate::config::{AppConfig, CustomTheme};
        use crate::theme::get_theme;

        let mut cfg = AppConfig::default();
        let custom = CustomTheme {
            id: "custom_cyberpunk".to_string(),
            name: "Cyberpunk Neon".to_string(),
            accent: "#00f0ff".to_string(),
            bg_canvas: "#0a0a14".to_string(),
            bg_sidebar: "#121220".to_string(),
            bg_elevated: "#1a1a2c".to_string(),
            text_primary: "#ffffff".to_string(),
            text_secondary: "#8888aa".to_string(),
            is_light: false,
        };
        cfg.custom_themes.push(custom);
        cfg.active_theme = "custom_cyberpunk".to_string();

        let json = serde_json::to_string(&cfg).expect("Should serialize config with custom theme");
        let deserialized: AppConfig = serde_json::from_str(&json).expect("Should deserialize config");
        assert_eq!(deserialized.active_theme, "custom_cyberpunk");
        assert_eq!(deserialized.custom_themes.len(), 1);

        let def = get_theme(&deserialized.active_theme, &deserialized.custom_themes);
        assert_eq!(def.name, "Cyberpunk Neon");
        assert_eq!(def.accent, "#00f0ff");
        assert!(def.is_custom);
    }
}
