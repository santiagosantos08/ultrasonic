use crate::api::NavidromeClient;
use crate::audio::AudioEngine;
use crate::config::{AppConfig, OfflineIndex};
use crate::models::*;
use crate::*;
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

pub struct AppState {
    pub config: AppConfig,
    pub offline: OfflineIndex,
    pub client: NavidromeClient,
    pub audio: AudioEngine,
    pub is_connected: bool,

    pub libraries: Vec<MusicFolder>,
    pub current_albums: Vec<Album>,
    pub featured_albums: Vec<Album>,
    pub current_artists: Vec<Artist>,
    pub selected_artist_albums: Vec<Album>,
    pub selected_artist_songs: Vec<Song>,
    pub current_songs: Vec<Song>,
    pub current_album_songs: Vec<Song>,
    pub current_playlist_songs: Vec<Song>,
    pub current_genres: Vec<Genre>,
    pub current_playlists: Vec<Playlist>,
    pub current_shares: Vec<Share>,
    pub queue: Vec<Song>,
    pub current_lyrics: Vec<StructuredLyricLine>,
    pub cover_cache: HashMap<String, slint::Image>,

    pub shuffle: bool,
    pub repeat_mode: String, // "off", "all", "one"
}

impl AppState {
    pub fn new() -> Self {
        let config = AppConfig::load();
        let offline = OfflineIndex::load();
        let client = NavidromeClient::new(&config.server_url, &config.username, &config.password);
        let audio = AudioEngine::new(config.volume);

        Self {
            config,
            offline,
            client,
            audio,
            is_connected: false,
            libraries: Vec::new(),
            current_albums: Vec::new(),
            featured_albums: Vec::new(),
            current_artists: Vec::new(),
            selected_artist_albums: Vec::new(),
            selected_artist_songs: Vec::new(),
            current_songs: Vec::new(),
            current_album_songs: Vec::new(),
            current_playlist_songs: Vec::new(),
            current_genres: Vec::new(),
            current_playlists: Vec::new(),
            current_shares: Vec::new(),
            queue: Vec::new(),
            current_lyrics: Vec::new(),
            cover_cache: HashMap::new(),
            shuffle: false,
            repeat_mode: "off".to_string(),
        }
    }

    pub fn active_library_ids(&self) -> Vec<String> {
        if self.config.selected_library_ids.is_empty() {
            self.libraries.iter().map(|l| l.id.clone()).collect()
        } else {
            self.config
                .selected_library_ids
                .iter()
                .filter(|id| self.libraries.iter().any(|l| &l.id == *id))
                .cloned()
                .collect()
        }
    }

    pub fn is_library_selected(&self, lib_id: &str) -> bool {
        self.config.selected_library_ids.is_empty()
            || self.config.selected_library_ids.iter().any(|x| x == lib_id)
    }

    pub fn active_libraries_summary(&self) -> String {
        let total = self.libraries.len();
        if total == 0 {
            return "No Libraries Connected".to_string();
        }
        let active = self
            .libraries
            .iter()
            .filter(|l| self.is_library_selected(&l.id))
            .count();
        if active == total {
            format!("All Libraries ({})", total)
        } else {
            format!("{} of {} Libraries Active", active, total)
        }
    }

    pub fn resolve_cover(&mut self, cover_id: Option<&str>) -> Option<slint::Image> {
        let cid = cover_id?.trim();
        if cid.is_empty() {
            return None;
        }
        if let Some(img) = self.cover_cache.get(cid) {
            return Some(img.clone());
        }

        #[cfg(not(target_arch = "wasm32"))]
        if self.is_connected {
            if let Some(bytes) = self.client.fetch_cover_art(cid, 300) {
                if let Some(decoded) = decode_cover_image(&bytes) {
                    self.cover_cache.insert(cid.to_string(), decoded.clone());
                    return Some(decoded);
                }
            }
        }

        None
    }

    pub fn to_slint_album(&mut self, alb: &Album) -> AlbumItem {
        let cover_opt = self.resolve_cover(alb.cover_art.as_deref());
        AlbumItem {
            id: alb.id.clone().into(),
            title: alb.display_title().into(),
            artist: alb.display_artist_name().into(),
            artist_id: alb.artist_id.clone().unwrap_or_default().into(),
            year: alb.year.map(|y| y.to_string()).unwrap_or_default().into(),
            genre: alb.genre.clone().unwrap_or_default().into(),
            song_count: alb.song_count as i32,
            duration_text: format_duration(alb.duration).into(),
            starred: alb.starred.is_some(),
            cover: cover_opt.clone().unwrap_or_default(),
            has_cover: cover_opt.is_some(),
        }
    }

    pub fn to_slint_song(&self, s: &Song, index: usize) -> SongItem {
        let is_playing = self
            .audio
            .current_song_id
            .as_deref()
            .map(|cid| cid == s.id)
            .unwrap_or(false);
        let is_downloaded = self.offline.is_downloaded(&s.id);
        let track_num = s
            .track
            .filter(|t| *t > 0)
            .map(|t| t.to_string())
            .unwrap_or_else(|| (index + 1).to_string());

        SongItem {
            id: s.id.clone().into(),
            track_num: track_num.into(),
            title: s.title.clone().into(),
            artist: s.display_artist_name().into(),
            artist_id: s.artist_id.clone().unwrap_or_default().into(),
            album: s.display_album_name().into(),
            album_id: s.album_id.clone().unwrap_or_default().into(),
            duration_text: format_duration(s.duration.unwrap_or(0)).into(),
            duration_secs: s.duration.unwrap_or(0) as i32,
            quality_badge: s.quality_badge().into(),
            starred: s.starred.is_some(),
            rating: s.user_rating.unwrap_or(0) as i32,
            is_downloaded,
            is_playing,
            play_count: s.play_count.unwrap_or(0) as i32,
            disc_number: s.disc_number.unwrap_or(1) as i32,
        }
    }

    pub fn find_song_by_id(&self, song_id: &str) -> Option<Song> {
        self.current_songs
            .iter()
            .chain(self.current_album_songs.iter())
            .chain(self.current_playlist_songs.iter())
            .chain(self.selected_artist_songs.iter())
            .chain(self.queue.iter())
            .find(|s| s.id == song_id)
            .cloned()
            .or_else(|| {
                self.offline
                    .tracks
                    .get(song_id)
                    .map(|entry| entry.song.clone())
            })
    }
}

pub fn sync_ui_from_state(ui: &AppWindow, state: &mut AppState) {
    ui.set_config_server_url(state.config.server_url.clone().into());
    ui.set_config_username(state.config.username.clone().into());
    ui.set_config_password(state.config.password.clone().into());
    ui.set_config_stream_format(state.config.stream_format.clone().into());
    ui.set_config_stream_bitrate(state.config.stream_max_bitrate as i32);
    ui.set_config_download_format(state.config.download_format.clone().into());
    ui.set_config_download_bitrate(state.config.download_max_bitrate as i32);
    ui.set_config_auto_download_streamed(state.config.auto_download_streamed);
    ui.set_config_scrobble_enabled(state.config.scrobble_enabled);
    ui.set_config_webui_enabled(state.config.webui_enabled);
    ui.set_config_webui_port(state.config.webui_port as i32);
    ui.set_volume(state.audio.volume);

    let lib_items: Vec<LibraryItem> = state
        .libraries
        .iter()
        .map(|l| LibraryItem {
            id: l.id.clone().into(),
            name: l.name.clone().into(),
            selected: state.is_library_selected(&l.id),
        })
        .collect();
    ui.set_libraries(ModelRc::new(VecModel::from(lib_items)));
    ui.set_active_libraries_summary(state.active_libraries_summary().into());

    let feat_cloned = state.featured_albums.clone();
    let feat_items: Vec<AlbumItem> = feat_cloned
        .iter()
        .map(|a| state.to_slint_album(a))
        .collect();
    ui.set_featured_albums(ModelRc::new(VecModel::from(feat_items)));

    let alb_cloned = state.current_albums.clone();
    let alb_items: Vec<AlbumItem> = alb_cloned
        .iter()
        .map(|a| state.to_slint_album(a))
        .collect();
    ui.set_albums(ModelRc::new(VecModel::from(alb_items)));

    let art_items: Vec<ArtistItem> = state
        .current_artists
        .iter()
        .map(|a| ArtistItem {
            id: a.id.clone().into(),
            name: a.name.clone().into(),
            album_count: a.album_count as i32,
            starred: a.starred.is_some(),
        })
        .collect();
    ui.set_artists(ModelRc::new(VecModel::from(art_items)));

    let art_alb_cloned = state.selected_artist_albums.clone();
    let art_alb_items: Vec<AlbumItem> = art_alb_cloned
        .iter()
        .map(|a| state.to_slint_album(a))
        .collect();
    ui.set_selected_artist_albums(ModelRc::new(VecModel::from(art_alb_items)));

    let art_song_items: Vec<SongItem> = state
        .selected_artist_songs
        .iter()
        .enumerate()
        .map(|(i, s)| state.to_slint_song(s, i))
        .collect();
    ui.set_selected_artist_top_songs(ModelRc::new(VecModel::from(art_song_items)));

    let song_items: Vec<SongItem> = state
        .current_songs
        .iter()
        .enumerate()
        .map(|(i, s)| state.to_slint_song(s, i))
        .collect();
    ui.set_songs(ModelRc::new(VecModel::from(song_items)));

    let alb_song_items: Vec<SongItem> = state
        .current_album_songs
        .iter()
        .enumerate()
        .map(|(i, s)| state.to_slint_song(s, i))
        .collect();
    ui.set_current_album_songs(ModelRc::new(VecModel::from(alb_song_items)));

    let genre_items: Vec<GenreItem> = state
        .current_genres
        .iter()
        .map(|g| GenreItem {
            name: g.value.clone().into(),
            song_count: g.song_count as i32,
            album_count: g.album_count as i32,
        })
        .collect();
    ui.set_genres(ModelRc::new(VecModel::from(genre_items)));

    let pl_items: Vec<PlaylistItem> = state
        .current_playlists
        .iter()
        .map(|p| PlaylistItem {
            id: p.id.clone().into(),
            name: p.name.clone().into(),
            comment: p.comment.clone().unwrap_or_default().into(),
            owner: p.owner.clone().unwrap_or_default().into(),
            public: p.public.unwrap_or(false),
            song_count: p.song_count as i32,
            duration_text: format_duration(p.duration).into(),
        })
        .collect();
    ui.set_playlists(ModelRc::new(VecModel::from(pl_items)));

    let pl_song_items: Vec<SongItem> = state
        .current_playlist_songs
        .iter()
        .enumerate()
        .map(|(i, s)| state.to_slint_song(s, i))
        .collect();
    ui.set_playlist_songs(ModelRc::new(VecModel::from(pl_song_items)));

    let dl_songs: Vec<SongItem> = state
        .offline
        .tracks
        .values()
        .enumerate()
        .map(|(i, entry)| state.to_slint_song(&entry.song, i))
        .collect();
    ui.set_downloaded_songs(ModelRc::new(VecModel::from(dl_songs)));
    ui.set_downloads_summary_text(state.offline.summary_string().into());

    let q_items: Vec<SongItem> = state
        .queue
        .iter()
        .enumerate()
        .map(|(i, s)| state.to_slint_song(s, i))
        .collect();
    ui.set_queue_songs(ModelRc::new(VecModel::from(q_items)));

    let share_items: Vec<ShareItem> = state
        .current_shares
        .iter()
        .map(|sh| ShareItem {
            id: sh.id.clone().into(),
            url: sh.url.clone().into(),
            description: sh.description.clone().unwrap_or_default().into(),
            visit_count: sh.visit_count.unwrap_or(0) as i32,
        })
        .collect();
    ui.set_shares(ModelRc::new(VecModel::from(share_items)));
}

pub fn refresh_catalog_for_active_libraries(ui: &AppWindow, state: &mut AppState) {
    if !state.is_connected {
        sync_ui_from_state(ui, state);
        return;
    }

    let lib_ids = state.active_library_ids();

    if let Ok(albs) = state.client.get_album_list2("newest", 40, 0, &lib_ids) {
        state.featured_albums = albs.clone();
        state.current_albums = albs;
    }
    if let Ok(rand_songs) = state.client.get_random_songs(80, &lib_ids) {
        state.current_songs = rand_songs;
    }
    if let Ok(arts) = state.client.get_artists(&lib_ids) {
        state.current_artists = arts;
    }
    if let Ok(genres) = state.client.get_genres() {
        state.current_genres = genres;
    }
    if let Ok(pls) = state.client.get_playlists() {
        state.current_playlists = pls;
    }
    if let Ok(shares) = state.client.get_shares() {
        state.current_shares = shares;
    }
    if let Ok(scan) = state.client.get_scan_status() {
        ui.set_scan_in_progress(scan.scanning);
        ui.set_scan_count(scan.count.unwrap_or(0) as i32);
        ui.set_scan_last_time(scan.last_scan.unwrap_or_else(|| "Idle".to_string()).into());
    }

    sync_ui_from_state(ui, state);
}

pub fn play_track_by_id(ui: &AppWindow, state: &mut AppState, song_id: &str) {
    let song = match state.find_song_by_id(song_id) {
        Some(s) => s,
        None => {
            ui.set_status_message("Track metadata not found".into());
            return;
        }
    };

    if state.queue.is_empty() {
        state.queue = if !state.current_songs.is_empty() {
            state.current_songs.clone()
        } else {
            vec![song.clone()]
        };
    } else if !state.queue.iter().any(|q| q.id == song.id) {
        state.queue.push(song.clone());
    }

    let duration_secs = song.duration.unwrap_or(0).max(1);
    let mut played_from_local = false;

    // 1. Play from local offline storage if already downloaded
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(local_path) = state.offline.local_file_path(&song.id) {
        match state.audio.play_file(&song.id, &local_path, duration_secs) {
            Ok(()) => {
                played_from_local = true;
            }
            Err(err) => {
                ui.set_status_message(format!("Local file error ({}), streaming instead...", err).into());
            }
        }
    }

    // 2. Otherwise stream from Navidrome
    if !played_from_local {
        if !state.is_connected {
            ui.set_status_message(
                "Not connected to Navidrome server (and track is not downloaded offline)".into(),
            );
            return;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            match state.client.fetch_stream_bytes(
                &song.id,
                &state.config.stream_format,
                state.config.stream_max_bitrate,
            ) {
                Ok(stream_bytes) => {
                    // 3. Auto-download what is streamed once if the toggle is enabled!
                    if state.config.auto_download_streamed && !state.offline.is_downloaded(&song.id)
                    {
                        let dl_bytes = if state.config.download_format != state.config.stream_format
                            || state.config.download_max_bitrate != state.config.stream_max_bitrate
                        {
                            state
                                .client
                                .fetch_download_bytes(
                                    &song.id,
                                    &state.config.download_format,
                                    state.config.download_max_bitrate,
                                )
                                .unwrap_or_else(|_| stream_bytes.clone())
                        } else {
                            stream_bytes.clone()
                        };

                        let dl_fmt = state.config.download_format.clone();
                        let dl_br = state.config.download_max_bitrate;
                        let _ = state.offline.store_track(song.clone(), &dl_bytes, &dl_fmt, dl_br);
                    }

                    if let Err(err) = state.audio.play_bytes(&song.id, stream_bytes, duration_secs)
                    {
                        ui.set_status_message(format!("Playback decode error: {}", err).into());
                        return;
                    }
                }
                Err(err) => {
                    ui.set_status_message(format!("Stream error: {}", err).into());
                    return;
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let url = state.client.stream_url(
                &song.id,
                &state.config.stream_format,
                state.config.stream_max_bitrate,
                0,
            );
            state.audio.play_url_wasm(&song.id, &url, duration_secs);
        }
    }

    // Send Navidrome Now Playing notification (`scrobble` with `submission=false`)
    if state.is_connected && state.config.scrobble_enabled {
        let _ = state.client.scrobble(&song.id, false);
    }

    // Load synchronized or embedded lyrics from Navidrome
    if state.is_connected {
        state.current_lyrics = state.client.get_lyrics_for_song(
            &song.id,
            &song.display_artist_name(),
            &song.title,
        );
    } else {
        state.current_lyrics.clear();
    }

    let lyric_items: Vec<LyricLineItem> = state
        .current_lyrics
        .iter()
        .enumerate()
        .map(|(idx, l)| LyricLineItem {
            time_ms: l.start.unwrap_or(0) as i32,
            text: l.value.clone().into(),
            active: idx == 0,
        })
        .collect();
    ui.set_lyrics_lines(ModelRc::new(VecModel::from(lyric_items)));

    // Update Now Playing UI properties
    let cover_opt = state.resolve_cover(song.cover_art.as_deref());
    ui.set_now_playing_cover(cover_opt.clone().unwrap_or_default());
    ui.set_now_playing_has_cover(cover_opt.is_some());
    ui.set_now_playing_id(song.id.clone().into());
    ui.set_now_playing_title(song.title.clone().into());
    ui.set_now_playing_artist(song.display_artist_name().into());
    ui.set_now_playing_album(song.display_album_name().into());
    ui.set_now_playing_album_id(song.album_id.clone().unwrap_or_default().into());
    ui.set_now_playing_artist_id(song.artist_id.clone().unwrap_or_default().into());
    ui.set_now_playing_starred(song.starred.is_some());
    ui.set_now_playing_rating(song.user_rating.unwrap_or(0) as i32);
    ui.set_now_playing_is_local(played_from_local || state.offline.is_downloaded(&song.id));

    let stream_badge = if played_from_local {
        format!("LOCAL · {}", song.quality_badge())
    } else if state.config.stream_format == "raw" && state.config.stream_max_bitrate == 0 {
        song.quality_badge()
    } else if state.config.stream_max_bitrate > 0 {
        format!(
            "{} {}k",
            state.config.stream_format.to_ascii_uppercase(),
            state.config.stream_max_bitrate
        )
    } else {
        state.config.stream_format.to_ascii_uppercase()
    };
    ui.set_now_playing_quality(stream_badge.into());
    ui.set_is_playing(true);
    ui.set_playback_current_time("0:00".into());
    ui.set_playback_total_time(format_duration(duration_secs).into());
    ui.set_playback_progress(0.0);

    if played_from_local {
        ui.set_status_message(format!("Playing '{}' from local offline storage", song.title).into());
    } else if state.config.auto_download_streamed {
        ui.set_status_message(format!("Streaming & auto-downloaded '{}'", song.title).into());
    } else {
        ui.set_status_message(format!("Streaming '{}'", song.title).into());
    }

    sync_ui_from_state(ui, state);
}

pub fn advance_track(ui: &AppWindow, state: &mut AppState, forward: bool) {
    if state.queue.is_empty() {
        return;
    }
    let current_id = state.audio.current_song_id.clone().unwrap_or_default();
    if state.repeat_mode == "one" && forward && !current_id.is_empty() {
        play_track_by_id(ui, state, &current_id);
        return;
    }

    let current_idx = state
        .queue
        .iter()
        .position(|s| s.id == current_id)
        .unwrap_or(0);

    let next_idx = if state.shuffle {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(1);
        (current_idx + 1 + nanos) % state.queue.len()
    } else if forward {
        if current_idx + 1 < state.queue.len() {
            current_idx + 1
        } else if state.repeat_mode == "all" {
            0
        } else {
            state.audio.stop();
            ui.set_is_playing(false);
            return;
        }
    } else if current_idx > 0 {
        current_idx - 1
    } else {
        0
    };

    if let Some(next_song) = state.queue.get(next_idx).cloned() {
        play_track_by_id(ui, state, &next_song.id);
    }
}

pub fn attach_callbacks_and_run(ui: &AppWindow, state: Rc<RefCell<AppState>>) {
    // Attempt connection on startup if credentials exist in config.json; otherwise open Settings view
    {
        let mut st = state.borrow_mut();
        if st.client.is_configured() {
            match st.client.ping() {
                Ok(ping_res) => {
                    st.is_connected = true;
                    ui.set_is_connected(true);
                    let srv_ver = ping_res
                        .server_version
                        .unwrap_or_else(|| ping_res.version.clone());
                    ui.set_server_version_badge(format!("Navidrome {}", srv_ver).into());
                    if let Ok(folders) = st.client.get_music_folders() {
                        st.libraries = folders;
                        if st.config.selected_library_ids.is_empty() {
                            st.config.selected_library_ids =
                                st.libraries.iter().map(|l| l.id.clone()).collect();
                        }
                    }
                    ui.set_active_view("listen_now".into());
                    ui.set_status_message("Connected to Navidrome server".into());
                    refresh_catalog_for_active_libraries(ui, &mut st);
                }
                Err(err) => {
                    st.is_connected = false;
                    ui.set_is_connected(false);
                    ui.set_active_view("settings".into());
                    ui.set_status_message(format!("Could not connect to Navidrome: {}", err).into());
                    sync_ui_from_state(ui, &mut st);
                }
            }
        } else {
            st.is_connected = false;
            ui.set_is_connected(false);
            ui.set_status_message("".into());
            sync_ui_from_state(ui, &mut st);
        }
    }

    // Navigate callback
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_navigate(move |view| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let view_str = view.to_string();
            ui.set_active_view(view.clone());
            let mut st = state.borrow_mut();
            if !st.is_connected {
                sync_ui_from_state(&ui, &mut st);
                return;
            }
            let lib_ids = st.active_library_ids();

            match view_str.as_str() {
                "listen_now" => {
                    refresh_catalog_for_active_libraries(&ui, &mut st);
                }
                "albums" => {
                    let sort_type = ui.get_album_sort_type().to_string();
                    if let Ok(albs) = st.client.get_album_list2(&sort_type, 100, 0, &lib_ids) {
                        st.current_albums = albs;
                    }
                    sync_ui_from_state(&ui, &mut st);
                }
                "favorites" => {
                    if let Ok(starred) = st.client.get_starred2(&lib_ids) {
                        st.current_songs = starred.song;
                    }
                    sync_ui_from_state(&ui, &mut st);
                }
                "songs" => {
                    if let Ok(songs) = st.client.get_random_songs(120, &lib_ids) {
                        st.current_songs = songs;
                    }
                    sync_ui_from_state(&ui, &mut st);
                }
                _ => {
                    sync_ui_from_state(&ui, &mut st);
                }
            }
        });
    }

    // Multi-Library Toggle (`toggle-library`) — allows selecting one or multiple visible libraries
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_library(move |lib_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let id_str = lib_id.to_string();

            if st.config.selected_library_ids.is_empty() {
                st.config.selected_library_ids =
                    st.libraries.iter().map(|l| l.id.clone()).collect();
            }

            if let Some(pos) = st
                .config
                .selected_library_ids
                .iter()
                .position(|x| x == &id_str)
            {
                if st.config.selected_library_ids.len() > 1 {
                    st.config.selected_library_ids.remove(pos);
                }
            } else {
                st.config.selected_library_ids.push(id_str);
            }

            st.config.save();
            let summary = st.active_libraries_summary();
            ui.set_status_message(format!("Visible libraries updated: {}", summary).into());
            refresh_catalog_for_active_libraries(&ui, &mut st);
        });
    }

    // Select All Libraries
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_select_all_libraries(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.selected_library_ids = st.libraries.iter().map(|l| l.id.clone()).collect();
            st.config.save();
            ui.set_status_message("All libraries selected".into());
            refresh_catalog_for_active_libraries(&ui, &mut st);
        });
    }

    // Album Sort
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_change_album_sort(move |sort_type| {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.set_album_sort_type(sort_type.clone());
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let sort_str = sort_type.to_string();
            let lib_ids = st.active_library_ids();
            if let Ok(albs) = st.client.get_album_list2(&sort_str, 100, 0, &lib_ids) {
                st.current_albums = albs;
            }
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Open Album Detail
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_open_album(move |album_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let aid = album_id.to_string();
            if let Ok(alb) = st.client.get_album(&aid) {
                let slint_alb = st.to_slint_album(&alb);
                ui.set_current_album_detail(slint_alb);
                st.current_album_songs = alb.song.clone();

                let notes = st
                    .client
                    .get_album_info2(&aid)
                    .ok()
                    .and_then(|info| info.notes)
                    .unwrap_or_default();
                ui.set_current_album_notes(notes.into());
                ui.set_active_view("album_detail".into());
                sync_ui_from_state(&ui, &mut st);
            }
        });
    }

    // Play Album
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_play_album(move |album_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let aid = album_id.to_string();
            if let Ok(alb) = st.client.get_album(&aid) {
                if !alb.song.is_empty() {
                    st.queue = alb.song.clone();
                    let first_id = alb.song[0].id.clone();
                    play_track_by_id(&ui, &mut st, &first_id);
                }
            }
        });
    }

    // Download Album
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_download_album(move |album_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let aid = album_id.to_string();
            if let Ok(alb) = st.client.get_album(&aid) {
                let dl_fmt = st.config.download_format.clone();
                let dl_br = st.config.download_max_bitrate;
                let mut count = 0;
                for song in &alb.song {
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Ok(bytes) = st.client.fetch_download_bytes(&song.id, &dl_fmt, dl_br) {
                        if st
                            .offline
                            .store_track(song.clone(), &bytes, &dl_fmt, dl_br)
                            .is_ok()
                        {
                            count += 1;
                        }
                    }
                }
                ui.set_status_message(
                    format!("Downloaded {} tracks from '{}'", count, alb.display_title()).into(),
                );
                sync_ui_from_state(&ui, &mut st);
            }
        });
    }

    // Open Artist
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_open_artist(move |artist_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let arid = artist_id.to_string();
            ui.set_selected_artist_id(arid.clone().into());
            ui.set_active_view("artists".into());

            if let Ok(detail) = st.client.get_artist(&arid) {
                ui.set_selected_artist_name(detail.name.clone().into());
                st.selected_artist_albums = detail.album;
                if let Ok(top) = st.client.get_top_songs(&detail.name, 30) {
                    st.selected_artist_songs = top;
                }
            }
            if let Ok(info) = st.client.get_artist_info2(&arid) {
                ui.set_selected_artist_bio(info.biography.unwrap_or_default().into());
            }
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Select Genre
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_select_genre(move |genre_name| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let gname = genre_name.to_string();
            ui.set_selected_genre_name(genre_name);
            let lib_ids = st.active_library_ids();
            if let Ok(songs) = st.client.get_songs_by_genre(&gname, 100, 0, &lib_ids) {
                st.current_songs = songs;
            }
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Search (`search3` scoped across selected libraries)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_run_search(move |query| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let q = query.to_string();
            let lib_ids = st.active_library_ids();
            if let Ok(res) = st.client.search3(&q, &lib_ids) {
                st.current_songs = res.song;
                if !res.album.is_empty() {
                    st.current_albums = res.album;
                }
            }
            ui.set_status_message(
                format!("Found {} songs matching '{}'", st.current_songs.len(), q).into(),
            );
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Play Song
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_play_song(move |song_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            play_track_by_id(&ui, &mut st, &song_id);
        });
    }

    // Queue Song
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_queue_song(move |song_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if let Some(song) = st.find_song_by_id(&song_id) {
                st.queue.push(song.clone());
                ui.set_status_message(format!("Added '{}' to Up Next", song.title).into());
                sync_ui_from_state(&ui, &mut st);
            }
        });
    }

    // Manual Download Song (`download` endpoint with configured download format & bitrate)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_download_song(move |song_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                ui.set_status_message("Connect to Navidrome server to download tracks".into());
                return;
            }
            if let Some(song) = st.find_song_by_id(&song_id) {
                let dl_fmt = st.config.download_format.clone();
                let dl_br = st.config.download_max_bitrate;

                #[cfg(not(target_arch = "wasm32"))]
                match st.client.fetch_download_bytes(&song.id, &dl_fmt, dl_br) {
                    Ok(bytes) => {
                        if st
                            .offline
                            .store_track(song.clone(), &bytes, &dl_fmt, dl_br)
                            .is_ok()
                        {
                            ui.set_status_message(
                                format!(
                                    "Downloaded '{}' ({} {})",
                                    song.title,
                                    dl_fmt.to_ascii_uppercase(),
                                    if dl_br == 0 {
                                        "Original".to_string()
                                    } else {
                                        format!("{}kbps", dl_br)
                                    }
                                )
                                .into(),
                            );
                            sync_ui_from_state(&ui, &mut st);
                        }
                    }
                    Err(err) => {
                        ui.set_status_message(format!("Download failed: {}", err).into());
                    }
                }
            }
        });
    }

    // Toggle Star (Song)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_star(move |song_id, star| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let sid = song_id.to_string();
            if st.is_connected {
                let _ = st.client.set_star(&sid, false, star);
            }
            let marker = if star {
                Some("starred".to_string())
            } else {
                None
            };
            for s in st.current_songs.iter_mut() {
                if s.id == sid {
                    s.starred = marker.clone();
                }
            }
            for s in st.current_album_songs.iter_mut() {
                if s.id == sid {
                    s.starred = marker.clone();
                }
            }
            for s in st.current_playlist_songs.iter_mut() {
                if s.id == sid {
                    s.starred = marker.clone();
                }
            }
            for s in st.queue.iter_mut() {
                if s.id == sid {
                    s.starred = marker.clone();
                }
            }
            if ui.get_now_playing_id() == song_id {
                ui.set_now_playing_starred(star);
            }
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Toggle Star (Album)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_album_star(move |album_id, star| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let aid = album_id.to_string();
            if st.is_connected {
                let _ = st.client.set_star(&aid, true, star);
            }
            let mut detail = ui.get_current_album_detail();
            if detail.id == album_id {
                detail.starred = star;
                ui.set_current_album_detail(detail);
            }
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Set 1-5 Star Rating (`setRating`)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_song_rating(move |song_id, rating| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            let sid = song_id.to_string();
            let r = (rating as u32).min(5);
            if st.is_connected {
                let _ = st.client.set_rating(&sid, r);
            }
            for s in st.current_songs.iter_mut() {
                if s.id == sid {
                    s.user_rating = Some(r);
                }
            }
            for s in st.queue.iter_mut() {
                if s.id == sid {
                    s.user_rating = Some(r);
                }
            }
            if ui.get_now_playing_id() == song_id {
                ui.set_now_playing_rating(rating);
            }
            ui.set_status_message(format!("Rated {} stars", rating).into());
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Playlist Management (`getPlaylist`, `createPlaylist`, `updatePlaylist`, `deletePlaylist`)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_select_playlist(move |pl_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let pid = pl_id.to_string();
            if let Ok(pl) = st.client.get_playlist(&pid) {
                ui.set_selected_playlist_id(pl.id.into());
                ui.set_selected_playlist_name(pl.name.into());
                ui.set_selected_playlist_count(pl.song_count as i32);
                ui.set_selected_playlist_duration(format_duration(pl.duration).into());
                st.current_playlist_songs = pl.entry;
                sync_ui_from_state(&ui, &mut st);
            }
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_create_new_playlist(move |name| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let trimmed = name.trim().to_string();
            if trimmed.is_empty() {
                return;
            }
            let _ = st.client.create_playlist(&trimmed, &[]);
            if let Ok(pls) = st.client.get_playlists() {
                st.current_playlists = pls;
            }
            ui.set_new_playlist_name("".into());
            ui.set_status_message(format!("Created playlist '{}'", trimmed).into());
            sync_ui_from_state(&ui, &mut st);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_add_current_song_to_playlist(move |pl_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let pid = pl_id.to_string();
            let Some(curr_id) = st.audio.current_song_id.clone() else { return };
            let Some(song) = st.find_song_by_id(&curr_id) else { return };

            let _ = st.client.update_playlist(&pid, &[curr_id], &[]);
            if let Ok(pl) = st.client.get_playlist(&pid) {
                st.current_playlist_songs = pl.entry;
            }
            ui.set_status_message(format!("Added '{}' to playlist", song.title).into());
            sync_ui_from_state(&ui, &mut st);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_remove_song_from_selected_playlist(move |row_idx| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let pid = ui.get_selected_playlist_id().to_string();
            let idx = row_idx as usize;
            let _ = st.client.update_playlist(&pid, &[], &[idx]);
            if let Ok(pl) = st.client.get_playlist(&pid) {
                st.current_playlist_songs = pl.entry;
            }
            sync_ui_from_state(&ui, &mut st);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_delete_selected_playlist(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let pid = ui.get_selected_playlist_id().to_string();
            if pid.is_empty() {
                return;
            }
            let _ = st.client.delete_playlist(&pid);
            if let Ok(pls) = st.client.get_playlists() {
                st.current_playlists = pls;
            }
            st.current_playlist_songs.clear();
            ui.set_selected_playlist_id("".into());
            ui.set_selected_playlist_name("".into());
            sync_ui_from_state(&ui, &mut st);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_download_selected_playlist(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let dl_fmt = st.config.download_format.clone();
            let dl_br = st.config.download_max_bitrate;
            let songs = st.current_playlist_songs.clone();
            let mut count = 0;
            for s in &songs {
                #[cfg(not(target_arch = "wasm32"))]
                if let Ok(bytes) = st.client.fetch_download_bytes(&s.id, &dl_fmt, dl_br) {
                    if st.offline.store_track(s.clone(), &bytes, &dl_fmt, dl_br).is_ok() {
                        count += 1;
                    }
                }
            }
            ui.set_status_message(format!("Downloaded {} playlist tracks", count).into());
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Transport Controls
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_transport_play_pause(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if st.audio.current_song_id.is_none() {
                if let Some(first) = st.current_songs.first().cloned() {
                    play_track_by_id(&ui, &mut st, &first.id);
                    return;
                }
            }
            let playing = st.audio.toggle_play_pause();
            ui.set_is_playing(playing);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_transport_next(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            advance_track(&ui, &mut st, true);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_transport_prev(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            advance_track(&ui, &mut st, false);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_transport_seek(move |val| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.audio.seek_ratio(val);
            let elapsed = st.audio.elapsed_secs() as u32;
            ui.set_playback_current_time(format_duration(elapsed).into());
        });
    }

    {
        let state = state.clone();
        ui.on_transport_volume(move |vol| {
            let mut st = state.borrow_mut();
            st.audio.set_volume(vol);
            st.config.volume = vol;
            st.config.save();
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_shuffle(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.shuffle = !st.shuffle;
            ui.set_shuffle_enabled(st.shuffle);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_cycle_repeat(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.repeat_mode = match st.repeat_mode.as_str() {
                "off" => "all".to_string(),
                "all" => "one".to_string(),
                _ => "off".to_string(),
            };
            ui.set_repeat_mode(st.repeat_mode.clone().into());
        });
    }

    // Play Queue Sync (`getPlayQueue` / `savePlayQueue` with string `current` ID)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_save_queue_to_server(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let ids: Vec<String> = st.queue.iter().map(|s| s.id.clone()).collect();
            let curr = st.audio.current_song_id.as_deref();
            let pos_ms = (st.audio.elapsed_secs() * 1000.0) as u64;
            if st.client.save_play_queue(&ids, curr, pos_ms).is_ok() {
                ui.set_status_message("Play queue synced to Navidrome server".into());
            }
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_load_queue_from_server(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            if let Ok(pq) = st.client.get_play_queue() {
                st.queue = pq.entry;
                if let Some(curr_id) = pq.current {
                    play_track_by_id(&ui, &mut st, &curr_id);
                }
                ui.set_status_message("Loaded play queue from server".into());
            }
            sync_ui_from_state(&ui, &mut st);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_clear_queue(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.queue.clear();
            sync_ui_from_state(&ui, &mut st);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_clear_downloads(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.offline.clear_all();
            ui.set_status_message("Cleared local offline downloads".into());
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Settings: Streaming & Download Rates and Auto-Download Toggle
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_stream_format(move |fmt| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.stream_format = fmt.to_string();
            st.config.save();
            ui.set_config_stream_format(fmt);
            ui.set_status_message(
                format!("Streaming format set to {}", st.config.stream_format).into(),
            );
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_stream_bitrate(move |br| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.stream_max_bitrate = br as u32;
            st.config.save();
            ui.set_config_stream_bitrate(br);
            ui.set_status_message(format!("Streaming max bitrate set to {} kbps", br).into());
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_download_format(move |fmt| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.download_format = fmt.to_string();
            st.config.save();
            ui.set_config_download_format(fmt);
            ui.set_status_message(
                format!("Download format set to {}", st.config.download_format).into(),
            );
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_set_download_bitrate(move |br| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.download_max_bitrate = br as u32;
            st.config.save();
            ui.set_config_download_bitrate(br);
            ui.set_status_message(format!("Download max bitrate set to {} kbps", br).into());
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_toggle_auto_download_streamed(move |enabled| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.auto_download_streamed = enabled;
            st.config.save();
            ui.set_config_auto_download_streamed(enabled);
            ui.set_status_message(
                if enabled {
                    "Auto-download on stream enabled: streamed tracks will be saved offline".into()
                } else {
                    "Auto-download on stream disabled".into()
                },
            );
        });
    }

    // Server Connection & Scan
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_save_and_connect(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.config.server_url = ui.get_config_server_url().to_string();
            st.config.username = ui.get_config_username().to_string();
            st.config.password = ui.get_config_password().to_string();
            st.config.save();

            st.client = NavidromeClient::new(
                &st.config.server_url,
                &st.config.username,
                &st.config.password,
            );

            match st.client.ping() {
                Ok(ping_res) => {
                    st.is_connected = true;
                    ui.set_is_connected(true);
                    let ver = ping_res
                        .server_version
                        .unwrap_or_else(|| ping_res.version.clone());
                    ui.set_server_version_badge(format!("Navidrome {}", ver).into());
                    if let Ok(folders) = st.client.get_music_folders() {
                        st.libraries = folders;
                        st.config.selected_library_ids =
                            st.libraries.iter().map(|l| l.id.clone()).collect();
                        st.config.save();
                    }
                    ui.set_active_view("listen_now".into());
                    ui.set_status_message("Connected to Navidrome server".into());
                    refresh_catalog_for_active_libraries(&ui, &mut st);
                }
                Err(err) => {
                    st.is_connected = false;
                    ui.set_is_connected(false);
                    ui.set_status_message(format!("Connection failed: {}", err).into());
                }
            }
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_disconnect_server(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            st.audio.stop();
            st.is_connected = false;
            ui.set_is_playing(false);
            ui.set_is_connected(false);
            ui.set_status_message("".into());
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_trigger_server_scan(move |full_scan| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                ui.set_status_message("Connect to Navidrome server first".into());
                return;
            }
            if let Ok(scan) = st.client.start_scan(full_scan) {
                ui.set_scan_in_progress(scan.scanning);
                ui.set_scan_count(scan.count.unwrap_or(0) as i32);
                ui.set_status_message(
                    if full_scan {
                        "Started Navidrome full media rescan".into()
                    } else {
                        "Started Navidrome quick scan".into()
                    },
                );
            }
        });
    }

    // Shares (`createShare` / `deleteShare`)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_share_current_album(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let alb = ui.get_current_album_detail();
            let aid = alb.id.to_string();
            let desc = format!("{} — {}", alb.title, alb.artist);
            if let Ok(mut new_shares) = st.client.create_share(&aid, &desc) {
                st.current_shares.append(&mut new_shares);
                ui.set_status_message(format!("Created Navidrome share link for '{}'", desc).into());
            }
            sync_ui_from_state(&ui, &mut st);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_delete_share(move |share_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let mut st = state.borrow_mut();
            if !st.is_connected {
                return;
            }
            let sid = share_id.to_string();
            let _ = st.client.delete_share(&sid);
            st.current_shares.retain(|s| s.id != sid);
            sync_ui_from_state(&ui, &mut st);
        });
    }

    // Periodic Playback Progress, Lyrics Sync, & Navidrome Scrobble Timer
    let timer = Timer::default();
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        timer.start(
            TimerMode::Repeated,
            Duration::from_millis(250),
            move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let mut st = state.borrow_mut();
                if !st.audio.is_playing {
                    return;
                }

                let elapsed = st.audio.elapsed_secs();
                let total = st.audio.duration_secs.max(1) as f32;
                let ratio = (elapsed / total).clamp(0.0, 1.0);

                ui.set_playback_progress(ratio);
                ui.set_playback_current_time(format_duration(elapsed as u32).into());

                // Highlight active synchronized lyric line
                let elapsed_ms = (elapsed * 1000.0) as i64;
                if !st.current_lyrics.is_empty() {
                    let mut active_idx = 0usize;
                    for (idx, line) in st.current_lyrics.iter().enumerate() {
                        if let Some(start) = line.start {
                            if elapsed_ms >= start {
                                active_idx = idx;
                            }
                        }
                    }
                    let items: Vec<LyricLineItem> = st
                        .current_lyrics
                        .iter()
                        .enumerate()
                        .map(|(idx, l)| LyricLineItem {
                            time_ms: l.start.unwrap_or(0) as i32,
                            text: l.value.clone().into(),
                            active: idx == active_idx,
                        })
                        .collect();
                    ui.set_lyrics_lines(ModelRc::new(VecModel::from(items)));
                }

                // Navidrome requires `scrobble(submission=true)` to mark a track as played
                if ratio >= 0.5 && !st.audio.scrobble_submitted_for_current {
                    st.audio.scrobble_submitted_for_current = true;
                    if st.is_connected && st.config.scrobble_enabled {
                        if let Some(ref sid) = st.audio.current_song_id {
                            let _ = st.client.scrobble(sid, true);
                        }
                    }
                }

                if st.audio.is_track_finished() {
                    advance_track(&ui, &mut st, true);
                }
            },
        );
    }
    std::mem::forget(timer);
}
