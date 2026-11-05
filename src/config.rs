use crate::models::Song;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server_url: String,
    pub username: String,
    pub password: String,
    /// Selected visible library IDs (`musicFolderId`). Empty means all accessible libraries are visible.
    pub selected_library_ids: Vec<String>,
    /// Streaming format (`"raw"`, `"flac"`, `"opus"`, `"mp3"`, `"aac"`).
    pub stream_format: String,
    /// Max streaming bitrate in kbps (`0` = no limit / original).
    pub stream_max_bitrate: u32,
    /// Download format (`"raw"`, `"flac"`, `"opus"`, `"mp3"`, `"aac"`).
    pub download_format: String,
    /// Max download bitrate in kbps (`0` = no limit / original).
    pub download_max_bitrate: u32,
    /// Toggle: automatically download tracks to local storage when streamed once.
    pub auto_download_streamed: bool,
    /// Send Navidrome scrobble notifications (`submission=false` on start, `submission=true` on play).
    pub scrobble_enabled: bool,
    /// Enable built-in HTTP WebUI server on Desktop.
    pub webui_enabled: bool,
    pub webui_port: u16,
    pub volume: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:4533".to_string(),
            username: String::new(),
            password: String::new(),
            selected_library_ids: Vec::new(),
            stream_format: "raw".to_string(),
            stream_max_bitrate: 0,
            download_format: "raw".to_string(),
            download_max_bitrate: 0,
            auto_download_streamed: true,
            scrobble_enabled: true,
            webui_enabled: true,
            webui_port: 4534,
            volume: 0.85,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadedTrackEntry {
    pub song: Song,
    pub file_name: String,
    pub size_bytes: u64,
    pub format: String,
    pub bitrate: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OfflineIndex {
    pub tracks: HashMap<String, DownloadedTrackEntry>,
}

impl AppConfig {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn app_data_dir() -> PathBuf {
        if let Some(proj) = directories::ProjectDirs::from("org", "ultrasonic", "Ultrasonic") {
            let dir = proj.data_local_dir().to_path_buf();
            let _ = std::fs::create_dir_all(&dir);
            dir
        } else {
            let dir = PathBuf::from("./.ultrasonic_data");
            let _ = std::fs::create_dir_all(&dir);
            dir
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn config_file_path() -> PathBuf {
        Self::app_data_dir().join("config.json")
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn downloads_dir() -> PathBuf {
        let dir = Self::app_data_dir().join("downloads");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn covers_dir() -> PathBuf {
        let dir = Self::app_data_dir().join("covers");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    pub fn load() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = Self::config_file_path();
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(cfg) = serde_json::from_str::<AppConfig>(&data) {
                    return cfg;
                }
            }
            Self::default()
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(win) = web_sys::window() {
                if let Ok(Some(storage)) = win.local_storage() {
                    if let Ok(Some(json)) = storage.get_item("ultrasonic_config") {
                        if let Ok(cfg) = serde_json::from_str::<AppConfig>(&json) {
                            return cfg;
                        }
                    }
                }
            }
            Self::default()
        }
    }

    pub fn save(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = Self::config_file_path();
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, json);
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(win) = web_sys::window() {
                if let Ok(Some(storage)) = win.local_storage() {
                    if let Ok(json) = serde_json::to_string(self) {
                        let _ = storage.set_item("ultrasonic_config", &json);
                    }
                }
            }
        }
    }
}

impl OfflineIndex {
    #[cfg(not(target_arch = "wasm32"))]
    fn index_path() -> PathBuf {
        AppConfig::downloads_dir().join("downloads_index.json")
    }

    pub fn load() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = Self::index_path();
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(idx) = serde_json::from_str::<OfflineIndex>(&data) {
                    return idx;
                }
            }
            Self::default()
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(win) = web_sys::window() {
                if let Ok(Some(storage)) = win.local_storage() {
                    if let Ok(Some(json)) = storage.get_item("ultrasonic_offline_index") {
                        if let Ok(idx) = serde_json::from_str::<OfflineIndex>(&json) {
                            return idx;
                        }
                    }
                }
            }
            Self::default()
        }
    }

    pub fn save(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = Self::index_path();
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, json);
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some(win) = web_sys::window() {
                if let Ok(Some(storage)) = win.local_storage() {
                    if let Ok(json) = serde_json::to_string(self) {
                        let _ = storage.set_item("ultrasonic_offline_index", &json);
                    }
                }
            }
        }
    }

    pub fn is_downloaded(&self, song_id: &str) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(entry) = self.tracks.get(song_id) {
                let file_path = AppConfig::downloads_dir().join(&entry.file_name);
                return file_path.exists();
            }
            false
        }

        #[cfg(target_arch = "wasm32")]
        {
            self.tracks.contains_key(song_id)
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn local_file_path(&self, song_id: &str) -> Option<PathBuf> {
        let entry = self.tracks.get(song_id)?;
        let path = AppConfig::downloads_dir().join(&entry.file_name);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    pub fn store_track(
        &mut self,
        song: Song,
        bytes: &[u8],
        format: &str,
        bitrate: u32,
    ) -> Result<(), String> {
        let ext = if format == "raw" || format.is_empty() {
            song.suffix.as_deref().unwrap_or("mp3")
        } else {
            format
        };
        let safe_id: String = song
            .id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let file_name = format!("{}.{}", safe_id, ext);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = AppConfig::downloads_dir().join(&file_name);
            std::fs::write(&path, bytes).map_err(|e| format!("Failed to write track: {}", e))?;
        }

        self.tracks.insert(
            song.id.clone(),
            DownloadedTrackEntry {
                song,
                file_name,
                size_bytes: bytes.len() as u64,
                format: format.to_string(),
                bitrate,
            },
        );
        self.save();
        Ok(())
    }

    pub fn clear_all(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            for entry in self.tracks.values() {
                let path = AppConfig::downloads_dir().join(&entry.file_name);
                let _ = std::fs::remove_file(path);
            }
        }
        self.tracks.clear();
        self.save();
    }

    pub fn summary_string(&self) -> String {
        let count = self.tracks.len();
        let total_bytes: u64 = self.tracks.values().map(|t| t.size_bytes).sum();
        let mb = (total_bytes as f64) / (1024.0 * 1024.0);
        format!("{} tracks stored locally ({:.1} MB)", count, mb)
    }
}
