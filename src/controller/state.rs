use crate::api::NavidromeClient;
use crate::audio::AudioEngine;
use crate::config::{AppConfig, OfflineIndex};
use crate::models::*;
use crate::*;
use slint::{ModelRc, VecModel};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

thread_local! {
    pub static CURRENT_STATE: RefCell<Option<Rc<RefCell<AppState>>>> = RefCell::new(None);
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
    pub pending_cover_fetches: HashSet<String>,
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
            pending_cover_fetches: HashSet::new(),
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

    ui.set_layout_override(state.config.layout_override.clone().into());
    ui.set_nav_show_listen_now(state.config.nav_show_listen_now);
    ui.set_nav_show_albums(state.config.nav_show_albums);
    ui.set_nav_show_artists(state.config.nav_show_artists);
    ui.set_nav_show_songs(state.config.nav_show_songs);
    ui.set_nav_show_playlists(state.config.nav_show_playlists);
    ui.set_nav_show_genres(state.config.nav_show_genres);
    ui.set_nav_show_favorites(state.config.nav_show_favorites);
    ui.set_nav_show_search(state.config.nav_show_search);

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
