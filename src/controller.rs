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

thread_local! {
    static CURRENT_STATE: RefCell<Option<Rc<RefCell<AppState>>>> = RefCell::new(None);
}

pub fn with_app_state<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AppState) -> R,
{
    CURRENT_STATE.with(|cell| {
        let borrowed = cell.borrow();
        let state_rc = borrowed.as_ref()?;
        let mut st = state_rc.borrow_mut();
        Some(f(&mut *st))
    })
}

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
    pub pending_cover_fetches: std::collections::HashSet<String>,
    pub has_new_covers: bool,
    pub play_generation: u64,

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
            pending_cover_fetches: std::collections::HashSet::new(),
            has_new_covers: false,
            play_generation: 0,
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
        if self.is_connected && !self.pending_cover_fetches.contains(cid) {
            self.pending_cover_fetches.insert(cid.to_string());
            let client = self.client.clone();
            let cid_owned = cid.to_string();
            std::thread::spawn(move || {
                let bytes_opt = client.fetch_cover_art(&cid_owned, 300);
                let _ = slint::invoke_from_event_loop(move || {
                    with_app_state(move |state| {
                        if let Some(bytes) = bytes_opt {
                            if let Some(decoded) = decode_cover_image(&bytes) {
                                state.cover_cache.insert(cid_owned.clone(), decoded);
                                state.has_new_covers = true;
                            }
                        }
                        state.pending_cover_fetches.remove(&cid_owned);
                    });
                });
            });
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

    ui.set_is_loading(true);
    let client = state.client.clone();
    let lib_ids = state.active_library_ids();
    let ui_weak = ui.as_weak();

    std::thread::spawn(move || {
        let albs = client.get_album_list2("newest", 40, 0, &lib_ids).ok();
        let rand_songs = client.get_random_songs(80, &lib_ids).ok();
        let arts = client.get_artists(&lib_ids).ok();
        let genres = client.get_genres().ok();
        let pls = client.get_playlists().ok();
        let shares = client.get_shares().ok();
        let scan = client.get_scan_status().ok();

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            with_app_state(move |st| {
                if let Some(albs) = albs {
                    st.featured_albums = albs.clone();
                    st.current_albums = albs;
                }
                if let Some(songs) = rand_songs {
                    st.current_songs = songs;
                }
                if let Some(arts) = arts {
                    st.current_artists = arts;
                }
                if let Some(genres) = genres {
                    st.current_genres = genres;
                }
                if let Some(pls) = pls {
                    st.current_playlists = pls;
                }
                if let Some(shares) = shares {
                    st.current_shares = shares;
                }
                if let Some(scan) = scan {
                    ui.set_scan_in_progress(scan.scanning);
                    ui.set_scan_count(scan.count.unwrap_or(0) as i32);
                    ui.set_scan_last_time(scan.last_scan.unwrap_or_else(|| "Idle".to_string()).into());
                }
                ui.set_is_loading(false);
                sync_ui_from_state(&ui, st);
            });
        });
    });
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

    // Update Now Playing UI immediately
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
    ui.set_now_playing_is_local(state.offline.is_downloaded(&song.id));
    ui.set_is_playing(true);
    ui.set_playback_current_time("0:00".into());
    ui.set_playback_total_time(format_duration(duration_secs).into());
    ui.set_playback_progress(0.0);

    state.play_generation = state.play_generation.wrapping_add(1);
    let my_gen = state.play_generation;

    // 1. Play from local offline storage if already downloaded
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(local_path) = state.offline.local_file_path(&song.id) {
        match state.audio.play_file(&song.id, &local_path, duration_secs) {
            Ok(()) => {
                ui.set_now_playing_is_local(true);
                ui.set_now_playing_quality(format!("LOCAL · {}", song.quality_badge()).into());
                ui.set_status_message(format!("Playing '{}' from local offline storage", song.title).into());
                sync_ui_from_state(ui, state);

                if state.is_connected {
                    let client = state.client.clone();
                    let sid = song.id.clone();
                    let s_artist = song.display_artist_name();
                    let s_title = song.title.clone();
                    let scrobble_enabled = state.config.scrobble_enabled;
                    let ui_weak = ui.as_weak();
                    std::thread::spawn(move || {
                        if scrobble_enabled {
                            let _ = client.scrobble(&sid, false);
                        }
                        let lyrics = client.get_lyrics_for_song(&sid, &s_artist, &s_title);
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                if st.play_generation != my_gen { return; }
                                st.current_lyrics = lyrics;
                                let lyric_items: Vec<LyricLineItem> = st
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
                            });
                        });
                    });
                }
                return;
            }
            Err(err) => {
                ui.set_status_message(format!("Local file error ({}), streaming instead...", err).into());
            }
        }
    }

    // 2. Otherwise stream from Navidrome in background
    if !state.is_connected {
        ui.set_is_playing(false);
        ui.set_status_message(
            "Not connected to Navidrome server (and track is not downloaded offline)".into(),
        );
        return;
    }

    ui.set_now_playing_quality("BUFFERING...".into());
    ui.set_status_message(format!("Buffering '{}'...", song.title).into());
    sync_ui_from_state(ui, state);

    let client = state.client.clone();
    let stream_format = state.config.stream_format.clone();
    let stream_max_bitrate = state.config.stream_max_bitrate;
    let auto_download = state.config.auto_download_streamed && !state.offline.is_downloaded(&song.id);
    let download_format = state.config.download_format.clone();
    let download_max_bitrate = state.config.download_max_bitrate;
    let scrobble_enabled = state.config.scrobble_enabled;
    let song_clone = song.clone();
    let ui_weak = ui.as_weak();

    std::thread::spawn(move || {
        let stream_bytes_res = client.fetch_stream_bytes(
            &song_clone.id,
            &stream_format,
            stream_max_bitrate,
        );

        let dl_bytes = if auto_download && stream_bytes_res.is_ok() {
            if download_format != stream_format || download_max_bitrate != stream_max_bitrate {
                client.fetch_download_bytes(&song_clone.id, &download_format, download_max_bitrate).ok()
            } else {
                stream_bytes_res.as_ref().ok().cloned()
            }
        } else {
            None
        };

        if scrobble_enabled {
            let _ = client.scrobble(&song_clone.id, false);
        }

        let lyrics = client.get_lyrics_for_song(
            &song_clone.id,
            &song_clone.display_artist_name(),
            &song_clone.title,
        );

        let _ = slint::invoke_from_event_loop(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            with_app_state(move |state| {
                if state.play_generation != my_gen {
                    return;
                }

                if let Some(dl_data) = dl_bytes {
                    let _ = state.offline.store_track(song_clone.clone(), &dl_data, &download_format, download_max_bitrate);
                }

                state.current_lyrics = lyrics;
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

                match stream_bytes_res {
                    Ok(stream_bytes) => {
                        if let Err(err) = state.audio.play_bytes(&song_clone.id, stream_bytes, duration_secs) {
                            ui.set_status_message(format!("Playback decode error: {}", err).into());
                            ui.set_is_playing(false);
                            return;
                        }

                        let stream_badge = if stream_format == "raw" && stream_max_bitrate == 0 {
                            song_clone.quality_badge()
                        } else if stream_max_bitrate > 0 {
                            format!("{} {}k", stream_format.to_ascii_uppercase(), stream_max_bitrate)
                        } else {
                            stream_format.to_ascii_uppercase()
                        };
                        ui.set_now_playing_quality(stream_badge.into());
                        ui.set_is_playing(true);
                        if auto_download {
                            ui.set_status_message(format!("Streaming & auto-downloaded '{}'", song_clone.title).into());
                        } else {
                            ui.set_status_message(format!("Streaming '{}'", song_clone.title).into());
                        }
                    }
                    Err(err) => {
                        ui.set_status_message(format!("Stream error: {}", err).into());
                        ui.set_is_playing(false);
                    }
                }

                sync_ui_from_state(&ui, state);
            });
        });
    });
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
    // Register global current state for thread-safe UI callbacks
    CURRENT_STATE.with(|cell| {
        *cell.borrow_mut() = Some(state.clone());
    });

    // Attempt connection on startup if credentials exist in config.json; otherwise open Settings view
    {
        let st = state.borrow();
        if st.client.is_configured() {
            ui.set_is_loading(true);
            ui.set_active_view("listen_now".into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                match client.ping() {
                    Ok(ping_res) => {
                        let folders = client.get_music_folders().ok().unwrap_or_default();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                st.is_connected = true;
                                ui.set_is_connected(true);
                                let srv_ver = ping_res
                                    .server_version
                                    .unwrap_or_else(|| ping_res.version.clone());
                                ui.set_server_version_badge(format!("Navidrome {}", srv_ver).into());
                                st.libraries = folders;
                                if st.config.selected_library_ids.is_empty() {
                                    st.config.selected_library_ids =
                                        st.libraries.iter().map(|l| l.id.clone()).collect();
                                }
                                ui.set_status_message("Connected to Navidrome server".into());
                                refresh_catalog_for_active_libraries(&ui, st);
                            });
                        });
                    }
                    Err(err) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                st.is_connected = false;
                                ui.set_is_connected(false);
                                ui.set_is_loading(false);
                                ui.set_active_view("settings".into());
                                ui.set_status_message(format!("Could not connect to Navidrome: {}", err).into());
                                sync_ui_from_state(&ui, st);
                            });
                        });
                    }
                }
            });
        } else {
            let mut st = state.borrow_mut();
            st.is_connected = false;
            ui.set_is_connected(false);
            ui.set_active_view("settings".into());
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
                    ui.set_is_loading(true);
                    let client = st.client.clone();
                    let sort_type = ui.get_album_sort_type().to_string();
                    let ui_weak = ui.as_weak();
                    std::thread::spawn(move || {
                        let albs = client.get_album_list2(&sort_type, 100, 0, &lib_ids).ok();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                if let Some(albs) = albs {
                                    st.current_albums = albs;
                                }
                                ui.set_is_loading(false);
                                sync_ui_from_state(&ui, st);
                            });
                        });
                    });
                }
                "favorites" => {
                    ui.set_is_loading(true);
                    let client = st.client.clone();
                    let ui_weak = ui.as_weak();
                    std::thread::spawn(move || {
                        let starred = client.get_starred2(&lib_ids).ok();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                if let Some(starred) = starred {
                                    st.current_songs = starred.song;
                                }
                                ui.set_is_loading(false);
                                sync_ui_from_state(&ui, st);
                            });
                        });
                    });
                }
                "songs" => {
                    ui.set_is_loading(true);
                    let client = st.client.clone();
                    let ui_weak = ui.as_weak();
                    std::thread::spawn(move || {
                        let songs = client.get_random_songs(120, &lib_ids).ok();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                if let Some(songs) = songs {
                                    st.current_songs = songs;
                                }
                                ui.set_is_loading(false);
                                sync_ui_from_state(&ui, st);
                            });
                        });
                    });
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
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            ui.set_is_loading(true);
            let sort_str = sort_type.to_string();
            let lib_ids = st.active_library_ids();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let albs = client.get_album_list2(&sort_str, 100, 0, &lib_ids).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(albs) = albs {
                            st.current_albums = albs;
                        }
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Open Album Detail
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_open_album(move |album_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let aid = album_id.to_string();
            ui.set_is_loading(true);
            ui.set_active_view("album_detail".into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let alb = client.get_album(&aid).ok();
                let notes = client
                    .get_album_info2(&aid)
                    .ok()
                    .and_then(|info| info.notes)
                    .unwrap_or_default();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(ref alb) = alb {
                            let slint_alb = st.to_slint_album(alb);
                            ui.set_current_album_detail(slint_alb);
                            st.current_album_songs = alb.song.clone();
                        }
                        ui.set_current_album_notes(notes.into());
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
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
            if ui.get_current_album_detail().id == album_id && !st.current_album_songs.is_empty() {
                st.queue = st.current_album_songs.clone();
                let first_id = st.current_album_songs[0].id.clone();
                play_track_by_id(&ui, &mut st, &first_id);
                return;
            }
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                if let Ok(alb) = client.get_album(&aid) {
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_weak.upgrade() else { return };
                        with_app_state(move |st| {
                            if !alb.song.is_empty() {
                                st.queue = alb.song.clone();
                                let first_id = alb.song[0].id.clone();
                                play_track_by_id(&ui, st, &first_id);
                            }
                        });
                    });
                }
            });
        });
    }

    // Download Album
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_download_album(move |album_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let aid = album_id.to_string();
            let dl_fmt = st.config.download_format.clone();
            let dl_br = st.config.download_max_bitrate;
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            ui.set_status_message("Starting album download...".into());

            std::thread::spawn(move || {
                if let Ok(alb) = client.get_album(&aid) {
                    let mut count = 0;
                    for song in &alb.song {
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Ok(bytes) = client.fetch_download_bytes(&song.id, &dl_fmt, dl_br) {
                            let s_clone = song.clone();
                            let fmt_c = dl_fmt.clone();
                            let _ = slint::invoke_from_event_loop(move || {
                                with_app_state(|st| {
                                    let _ = st.offline.store_track(s_clone, &bytes, &fmt_c, dl_br);
                                });
                            });
                            count += 1;
                        }
                    }
                    let alb_title = alb.display_title();
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_weak.upgrade() else { return };
                        with_app_state(move |st| {
                            ui.set_status_message(format!("Downloaded {} tracks from '{}'", count, alb_title).into());
                            sync_ui_from_state(&ui, st);
                        });
                    });
                }
            });
        });
    }

    // Open Artist
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_open_artist(move |artist_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let arid = artist_id.to_string();
            ui.set_selected_artist_id(arid.clone().into());
            ui.set_is_loading(true);
            ui.set_active_view("artists".into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let detail = client.get_artist(&arid).ok();
                let top = if let Some(ref d) = detail {
                    client.get_top_songs(&d.name, 30).ok()
                } else {
                    None
                };
                let bio = client
                    .get_artist_info2(&arid)
                    .ok()
                    .and_then(|info| info.biography)
                    .unwrap_or_default();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(detail) = detail {
                            ui.set_selected_artist_name(detail.name.clone().into());
                            st.selected_artist_albums = detail.album;
                        }
                        if let Some(top) = top {
                            st.selected_artist_songs = top;
                        }
                        ui.set_selected_artist_bio(bio.into());
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Select Genre
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_select_genre(move |genre_name| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let gname = genre_name.to_string();
            ui.set_selected_genre_name(genre_name);
            ui.set_is_loading(true);
            let lib_ids = st.active_library_ids();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let songs = client.get_songs_by_genre(&gname, 100, 0, &lib_ids).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(songs) = songs {
                            st.current_songs = songs;
                        }
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Search (`search3` scoped across selected libraries)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_run_search(move |query| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let q = query.to_string();
            if q.trim().is_empty() {
                return;
            }
            ui.set_is_loading(true);
            let lib_ids = st.active_library_ids();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let res = client.search3(&q, &lib_ids).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(res) = res {
                            st.current_songs = res.song;
                            if !res.album.is_empty() {
                                st.current_albums = res.album;
                            }
                            ui.set_status_message(
                                format!("Found {} songs matching '{}'", st.current_songs.len(), q).into(),
                            );
                        }
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
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
            let st = state.borrow();
            if !st.is_connected {
                ui.set_status_message("Connect to Navidrome server to download tracks".into());
                return;
            }
            if let Some(song) = st.find_song_by_id(&song_id) {
                let dl_fmt = st.config.download_format.clone();
                let dl_br = st.config.download_max_bitrate;
                let client = st.client.clone();
                let ui_weak = ui.as_weak();
                ui.set_status_message(format!("Downloading '{}'...", song.title).into());

                std::thread::spawn(move || {
                    #[cfg(not(target_arch = "wasm32"))]
                    match client.fetch_download_bytes(&song.id, &dl_fmt, dl_br) {
                        Ok(bytes) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                let Some(ui) = ui_weak.upgrade() else { return };
                                with_app_state(move |st| {
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
                                        sync_ui_from_state(&ui, st);
                                    }
                                });
                            });
                        }
                        Err(err) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                let Some(ui) = ui_weak.upgrade() else { return };
                                ui.set_status_message(format!("Download failed: {}", err).into());
                            });
                        }
                    }
                });
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
                let client = st.client.clone();
                let sid_c = sid.clone();
                std::thread::spawn(move || {
                    let _ = client.set_star(&sid_c, false, star);
                });
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
                let client = st.client.clone();
                let aid_c = aid.clone();
                std::thread::spawn(move || {
                    let _ = client.set_star(&aid_c, true, star);
                });
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
                let client = st.client.clone();
                let sid_c = sid.clone();
                std::thread::spawn(move || {
                    let _ = client.set_rating(&sid_c, r);
                });
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
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let pid = pl_id.to_string();
            ui.set_is_loading(true);
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let pl = client.get_playlist(&pid).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pl) = pl {
                            ui.set_selected_playlist_id(pl.id.into());
                            ui.set_selected_playlist_name(pl.name.into());
                            ui.set_selected_playlist_count(pl.song_count as i32);
                            ui.set_selected_playlist_duration(format_duration(pl.duration).into());
                            st.current_playlist_songs = pl.entry;
                        }
                        ui.set_is_loading(false);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_create_new_playlist(move |name| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let trimmed = name.trim().to_string();
            if trimmed.is_empty() {
                return;
            }
            ui.set_new_playlist_name("".into());
            ui.set_status_message(format!("Creating playlist '{}'...", trimmed).into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let _ = client.create_playlist(&trimmed, &[]);
                let pls = client.get_playlists().ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pls) = pls {
                            st.current_playlists = pls;
                        }
                        ui.set_status_message(format!("Created playlist '{}'", trimmed).into());
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_add_current_song_to_playlist(move |pl_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let pid = pl_id.to_string();
            let Some(curr_id) = st.audio.current_song_id.clone() else { return };
            let Some(song) = st.find_song_by_id(&curr_id) else { return };
            let song_title = song.title.clone();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let _ = client.update_playlist(&pid, &[curr_id], &[]);
                let pl = client.get_playlist(&pid).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pl) = pl {
                            st.current_playlist_songs = pl.entry;
                        }
                        ui.set_status_message(format!("Added '{}' to playlist", song_title).into());
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_remove_song_from_selected_playlist(move |row_idx| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let pid = ui.get_selected_playlist_id().to_string();
            let idx = row_idx as usize;
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let _ = client.update_playlist(&pid, &[], &[idx]);
                let pl = client.get_playlist(&pid).ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pl) = pl {
                            st.current_playlist_songs = pl.entry;
                        }
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_delete_selected_playlist(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let pid = ui.get_selected_playlist_id().to_string();
            if pid.is_empty() {
                return;
            }
            ui.set_selected_playlist_id("".into());
            ui.set_selected_playlist_name("".into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let _ = client.delete_playlist(&pid);
                let pls = client.get_playlists().ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pls) = pls {
                            st.current_playlists = pls;
                        }
                        st.current_playlist_songs.clear();
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_download_selected_playlist(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let dl_fmt = st.config.download_format.clone();
            let dl_br = st.config.download_max_bitrate;
            let songs = st.current_playlist_songs.clone();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            ui.set_status_message(format!("Downloading {} playlist tracks...", songs.len()).into());

            std::thread::spawn(move || {
                let mut count = 0;
                for s in songs {
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Ok(bytes) = client.fetch_download_bytes(&s.id, &dl_fmt, dl_br) {
                        let s_clone = s.clone();
                        let fmt_c = dl_fmt.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            with_app_state(|st| {
                                let _ = st.offline.store_track(s_clone, &bytes, &fmt_c, dl_br);
                            });
                        });
                        count += 1;
                    }
                }
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        ui.set_status_message(format!("Downloaded {} playlist tracks", count).into());
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
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
            let curr = st.audio.current_song_id.clone();
            let pos_ms = (st.audio.elapsed_secs() * 1000.0) as u64;
            let client = st.client.clone();
            std::thread::spawn(move || {
                let _ = client.save_play_queue(&ids, curr.as_deref(), pos_ms);
            });
            ui.set_status_message("Play queue synced to Navidrome server".into());
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_load_queue_from_server(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            ui.set_status_message("Loading play queue from server...".into());
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let pq = client.get_play_queue().ok();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        if let Some(pq) = pq {
                            st.queue = pq.entry;
                            if let Some(curr_id) = pq.current {
                                play_track_by_id(&ui, st, &curr_id);
                            }
                            ui.set_status_message("Loaded play queue from server".into());
                            sync_ui_from_state(&ui, st);
                        }
                    });
                });
            });
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
        ui.on_save_and_connect(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            ui.set_is_loading(true);
            let server_url = ui.get_config_server_url().to_string();
            let username = ui.get_config_username().to_string();
            let password = ui.get_config_password().to_string();
            let ui_weak = ui.as_weak();

            std::thread::spawn(move || {
                let client = NavidromeClient::new(&server_url, &username, &password);
                match client.ping() {
                    Ok(ping_res) => {
                        let folders = client.get_music_folders().ok().unwrap_or_default();
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                st.config.server_url = server_url;
                                st.config.username = username;
                                st.config.password = password;
                                st.config.save();
                                st.client = client;
                                st.is_connected = true;
                                ui.set_is_connected(true);
                                let ver = ping_res
                                    .server_version
                                    .unwrap_or_else(|| ping_res.version.clone());
                                ui.set_server_version_badge(format!("Navidrome {}", ver).into());
                                st.libraries = folders;
                                st.config.selected_library_ids =
                                    st.libraries.iter().map(|l| l.id.clone()).collect();
                                st.config.save();
                                ui.set_active_view("listen_now".into());
                                ui.set_status_message("Connected to Navidrome server".into());
                                refresh_catalog_for_active_libraries(&ui, st);
                            });
                        });
                    }
                    Err(err) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            let Some(ui) = ui_weak.upgrade() else { return };
                            with_app_state(move |st| {
                                st.is_connected = false;
                                ui.set_is_connected(false);
                                ui.set_is_loading(false);
                                ui.set_status_message(format!("Connection failed: {}", err).into());
                            });
                        });
                    }
                }
            });
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
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            ui.set_status_message(
                if full_scan {
                    "Requesting Navidrome full media rescan...".into()
                } else {
                    "Requesting Navidrome quick scan...".into()
                },
            );
            std::thread::spawn(move || {
                if let Ok(scan) = client.start_scan(full_scan) {
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_weak.upgrade() else { return };
                        ui.set_scan_in_progress(scan.scanning);
                        ui.set_scan_count(scan.count.unwrap_or(0) as i32);
                        ui.set_status_message(
                            if full_scan {
                                "Started Navidrome full media rescan".into()
                            } else {
                                "Started Navidrome quick scan".into()
                            },
                        );
                    });
                }
            });
        });
    }

    // Shares (`createShare` / `deleteShare`)
    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_share_current_album(move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let alb = ui.get_current_album_detail();
            let aid = alb.id.to_string();
            let desc = format!("{} — {}", alb.title, alb.artist);
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                if let Ok(mut new_shares) = client.create_share(&aid, &desc) {
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_weak.upgrade() else { return };
                        with_app_state(move |st| {
                            st.current_shares.append(&mut new_shares);
                            ui.set_status_message(format!("Created Navidrome share link for '{}'", desc).into());
                            sync_ui_from_state(&ui, st);
                        });
                    });
                }
            });
        });
    }

    {
        let ui_weak = ui.as_weak();
        let state = state.clone();
        ui.on_delete_share(move |share_id| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let st = state.borrow();
            if !st.is_connected {
                return;
            }
            let sid = share_id.to_string();
            let client = st.client.clone();
            let ui_weak = ui.as_weak();
            std::thread::spawn(move || {
                let _ = client.delete_share(&sid);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return };
                    with_app_state(move |st| {
                        st.current_shares.retain(|s| s.id != sid);
                        sync_ui_from_state(&ui, st);
                    });
                });
            });
        });
    }

    // Periodic Playback Progress, Lyrics Sync, Shimmer Pulse & Async Covers Timer
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

                // 1. Shimmer animation pulse toggle for skeleton loaders
                ui.set_pulse_phase(!ui.get_pulse_phase());

                // 2. Progressive cover art loading: when new covers finish downloading in background, update models
                if st.has_new_covers {
                    st.has_new_covers = false;
                    sync_ui_from_state(&ui, &mut st);
                }

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
                            let client = st.client.clone();
                            let sid_c = sid.clone();
                            std::thread::spawn(move || {
                                let _ = client.scrobble(&sid_c, true);
                            });
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
