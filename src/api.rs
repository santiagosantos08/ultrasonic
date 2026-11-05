use crate::config::AppConfig;
use crate::models::*;
use md5::{Digest, Md5};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct NavidromeClient {
    pub base_url: String,
    pub username: String,
    pub password: String,
    #[cfg(not(target_arch = "wasm32"))]
    http: ureq::Agent,
}

impl NavidromeClient {
    pub fn new(base_url: &str, username: &str, password: &str) -> Self {
        let trimmed = base_url.trim().trim_end_matches('/').to_string();
        #[cfg(not(target_arch = "wasm32"))]
        let http = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(20))
            .timeout_write(Duration::from_secs(10))
            .user_agent("Ultrasonic/0.1.0 (Navidrome; OpenSubsonic)")
            .build();

        Self {
            base_url: trimmed,
            username: username.trim().to_string(),
            password: password.to_string(),
            #[cfg(not(target_arch = "wasm32"))]
            http,
        }
    }

    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty() && !self.username.is_empty()
    }

    fn generate_salt() -> String {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(987654321);
        format!("us{:x}", nanos & 0xffff_ffff)
    }

    fn compute_token(password: &str, salt: &str) -> String {
        let mut hasher = Md5::new();
        hasher.update(password.as_bytes());
        hasher.update(salt.as_bytes());
        let digest = hasher.finalize();
        format!("{:x}", digest)
    }

    /// Builds a full Subsonic / OpenSubsonic REST endpoint URL with MD5 token + salt auth
    /// and optional repeated parameters (such as multiple `musicFolderId`s).
    pub fn build_url(&self, endpoint: &str, params: &[(&str, String)]) -> String {
        let salt = Self::generate_salt();
        let token = Self::compute_token(&self.password, &salt);
        let endpoint_clean = endpoint.trim_start_matches('/');

        let mut query = format!(
            "u={}&t={}&s={}&v=1.16.1&c=Ultrasonic&f=json",
            urlencoding::encode(&self.username),
            token,
            salt
        );

        for (k, v) in params {
            query.push('&');
            query.push_str(&urlencoding::encode(k));
            query.push('=');
            query.push_str(&urlencoding::encode(v));
        }

        format!("{}/rest/{}?{}", self.base_url, endpoint_clean, query)
    }

    fn append_library_filters<'a>(
        params: &mut Vec<(&'a str, String)>,
        library_ids: &[String],
    ) {
        for lib_id in library_ids {
            if !lib_id.is_empty() {
                params.push(("musicFolderId", lib_id.clone()));
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn call_api(&self, endpoint: &str, params: &[(&str, String)]) -> Result<SubsonicResponse, String> {
        if !self.is_configured() {
            return Err("Server URL or username is empty".to_string());
        }
        let url = self.build_url(endpoint, params);
        let resp = self
            .http
            .get(&url)
            .call()
            .map_err(|e| format!("Network error calling {}: {}", endpoint, e))?;

        let body = resp
            .into_string()
            .map_err(|e| format!("Failed reading response from {}: {}", endpoint, e))?;

        let env: SubsonicEnvelope = serde_json::from_str(&body)
            .map_err(|e| format!("Invalid Subsonic JSON on {}: {}", endpoint, e))?;

        if env.response.status != "ok" {
            let err_msg = env
                .response
                .error
                .map(|er| format!("Subsonic Error {}: {}", er.code, er.message))
                .unwrap_or_else(|| "Unknown Subsonic API error".to_string());
            return Err(err_msg);
        }

        Ok(env.response)
    }

    #[cfg(target_arch = "wasm32")]
    fn call_api(&self, _endpoint: &str, _params: &[(&str, String)]) -> Result<SubsonicResponse, String> {
        Err("Use async Wasm fetch or demo mode".to_string())
    }

    pub fn ping(&self) -> Result<SubsonicResponse, String> {
        self.call_api("ping", &[])
    }

    pub fn get_open_subsonic_extensions(&self) -> Result<Vec<OpenSubsonicExtension>, String> {
        let res = self.call_api("getOpenSubsonicExtensions", &[])?;
        Ok(res.open_subsonic_extensions.unwrap_or_default())
    }

    pub fn get_scan_status(&self) -> Result<ScanStatus, String> {
        let res = self.call_api("getScanStatus", &[])?;
        Ok(res.scan_status.unwrap_or_default())
    }

    /// Starts a Navidrome media scan (`fullScan=true` forces a full rescan in Navidrome).
    pub fn start_scan(&self, full_scan: bool) -> Result<ScanStatus, String> {
        let params = vec![("fullScan", if full_scan { "true" } else { "false" }.to_string())];
        let res = self.call_api("startScan", &params)?;
        Ok(res.scan_status.unwrap_or_default())
    }

    /// Fetches all libraries (Music Folders) accessible to the authenticated Navidrome user.
    pub fn get_music_folders(&self) -> Result<Vec<MusicFolder>, String> {
        let res = self.call_api("getMusicFolders", &[])?;
        Ok(res
            .music_folders
            .map(|w| w.music_folder)
            .unwrap_or_default())
    }

    /// Fetches albums using `getAlbumList2` scoped to the user's selected `musicFolderId`s.
    pub fn get_album_list2(
        &self,
        list_type: &str,
        size: u32,
        offset: u32,
        library_ids: &[String],
    ) -> Result<Vec<Album>, String> {
        let mut params = vec![
            ("type", list_type.to_string()),
            ("size", size.to_string()),
            ("offset", offset.to_string()),
        ];
        Self::append_library_filters(&mut params, library_ids);
        let res = self.call_api("getAlbumList2", &params)?;
        Ok(res.album_list2.map(|w| w.album).unwrap_or_default())
    }

    pub fn get_album(&self, album_id: &str) -> Result<Album, String> {
        let params = vec![("id", album_id.to_string())];
        let res = self.call_api("getAlbum", &params)?;
        res.album.ok_or_else(|| "Album not found".to_string())
    }

    pub fn get_album_info2(&self, album_id: &str) -> Result<AlbumInfo, String> {
        let params = vec![("id", album_id.to_string())];
        let res = self.call_api("getAlbumInfo2", &params)?;
        Ok(res.album_info2.unwrap_or_default())
    }

    pub fn get_artists(&self, library_ids: &[String]) -> Result<Vec<Artist>, String> {
        let mut params = Vec::new();
        Self::append_library_filters(&mut params, library_ids);
        let res = self.call_api("getArtists", &params)?;
        let mut all_artists = Vec::new();
        if let Some(wrapper) = res.artists {
            for idx in wrapper.index {
                for art in idx.artist {
                    all_artists.push(art);
                }
            }
        }
        Ok(all_artists)
    }

    pub fn get_artist(&self, artist_id: &str) -> Result<ArtistDetail, String> {
        let params = vec![("id", artist_id.to_string())];
        let res = self.call_api("getArtist", &params)?;
        res.artist.ok_or_else(|| "Artist not found".to_string())
    }

    pub fn get_artist_info2(&self, artist_id: &str) -> Result<ArtistInfo, String> {
        let params = vec![("id", artist_id.to_string())];
        let res = self.call_api("getArtistInfo2", &params)?;
        Ok(res.artist_info2.unwrap_or_default())
    }

    pub fn get_top_songs(&self, artist_name: &str, count: u32) -> Result<Vec<Song>, String> {
        let params = vec![
            ("artist", artist_name.to_string()),
            ("count", count.to_string()),
        ];
        let res = self.call_api("getTopSongs", &params)?;
        Ok(res.top_songs.map(|w| w.song).unwrap_or_default())
    }

    pub fn get_random_songs(&self, size: u32, library_ids: &[String]) -> Result<Vec<Song>, String> {
        let mut params = vec![("size", size.to_string())];
        Self::append_library_filters(&mut params, library_ids);
        let res = self.call_api("getRandomSongs", &params)?;
        Ok(res.random_songs.map(|w| w.song).unwrap_or_default())
    }

    pub fn get_genres(&self) -> Result<Vec<Genre>, String> {
        let res = self.call_api("getGenres", &[])?;
        Ok(res.genres.map(|w| w.genre).unwrap_or_default())
    }

    pub fn get_songs_by_genre(
        &self,
        genre: &str,
        count: u32,
        offset: u32,
        library_ids: &[String],
    ) -> Result<Vec<Song>, String> {
        let mut params = vec![
            ("genre", genre.to_string()),
            ("count", count.to_string()),
            ("offset", offset.to_string()),
        ];
        Self::append_library_filters(&mut params, library_ids);
        let res = self.call_api("getSongsByGenre", &params)?;
        Ok(res.songs_by_genre.map(|w| w.song).unwrap_or_default())
    }

    pub fn get_starred2(&self, library_ids: &[String]) -> Result<StarredWrapper, String> {
        let mut params = Vec::new();
        Self::append_library_filters(&mut params, library_ids);
        let res = self.call_api("getStarred2", &params)?;
        Ok(res.starred2.unwrap_or_default())
    }

    pub fn search3(&self, query: &str, library_ids: &[String]) -> Result<SearchResult3, String> {
        let mut params = vec![
            ("query", query.to_string()),
            ("artistCount", "30".to_string()),
            ("albumCount", "30".to_string()),
            ("songCount", "100".to_string()),
        ];
        Self::append_library_filters(&mut params, library_ids);
        let res = self.call_api("search3", &params)?;
        Ok(res.search_result3.unwrap_or_default())
    }

    pub fn get_playlists(&self) -> Result<Vec<Playlist>, String> {
        let res = self.call_api("getPlaylists", &[])?;
        Ok(res.playlists.map(|w| w.playlist).unwrap_or_default())
    }

    pub fn get_playlist(&self, playlist_id: &str) -> Result<Playlist, String> {
        let params = vec![("id", playlist_id.to_string())];
        let res = self.call_api("getPlaylist", &params)?;
        res.playlist.ok_or_else(|| "Playlist not found".to_string())
    }

    pub fn create_playlist(&self, name: &str, song_ids: &[String]) -> Result<Option<Playlist>, String> {
        let mut params = vec![("name", name.to_string())];
        for sid in song_ids {
            params.push(("songId", sid.clone()));
        }
        let res = self.call_api("createPlaylist", &params)?;
        Ok(res.playlist)
    }

    pub fn update_playlist(
        &self,
        playlist_id: &str,
        song_ids_to_add: &[String],
        song_indexes_to_remove: &[usize],
    ) -> Result<(), String> {
        let mut params = vec![("playlistId", playlist_id.to_string())];
        for sid in song_ids_to_add {
            params.push(("songIdToAdd", sid.clone()));
        }
        for idx in song_indexes_to_remove {
            params.push(("songIndexToRemove", idx.to_string()));
        }
        let _ = self.call_api("updatePlaylist", &params)?;
        Ok(())
    }

    pub fn delete_playlist(&self, playlist_id: &str) -> Result<(), String> {
        let params = vec![("id", playlist_id.to_string())];
        let _ = self.call_api("deletePlaylist", &params)?;
        Ok(())
    }

    pub fn set_star(&self, id: &str, is_album: bool, star: bool) -> Result<(), String> {
        let key = if is_album { "albumId" } else { "id" };
        let endpoint = if star { "star" } else { "unstar" };
        let params = vec![(key, id.to_string())];
        let _ = self.call_api(endpoint, &params)?;
        Ok(())
    }

    pub fn set_rating(&self, id: &str, rating: u32) -> Result<(), String> {
        let params = vec![
            ("id", id.to_string()),
            ("rating", rating.min(5).to_string()),
        ];
        let _ = self.call_api("setRating", &params)?;
        Ok(())
    }

    /// Navidrome requires `scrobble` with `submission=true` to increment play count & last played timestamp.
    pub fn scrobble(&self, song_id: &str, submission: bool) -> Result<(), String> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let params = vec![
            ("id", song_id.to_string()),
            ("time", now_ms.to_string()),
            ("submission", if submission { "true" } else { "false" }.to_string()),
        ];
        let _ = self.call_api("scrobble", &params)?;
        Ok(())
    }

    pub fn get_play_queue(&self) -> Result<PlayQueue, String> {
        let res = self.call_api("getPlayQueue", &[])?;
        Ok(res.play_queue.unwrap_or_default())
    }

    pub fn save_play_queue(
        &self,
        song_ids: &[String],
        current_id: Option<&str>,
        position_ms: u64,
    ) -> Result<(), String> {
        let mut params = Vec::new();
        for sid in song_ids {
            params.push(("id", sid.clone()));
        }
        if let Some(curr) = current_id {
            if !curr.is_empty() {
                params.push(("current", curr.to_string()));
            }
        }
        params.push(("position", position_ms.to_string()));
        let _ = self.call_api("savePlayQueue", &params)?;
        Ok(())
    }

    pub fn get_shares(&self) -> Result<Vec<Share>, String> {
        let res = self.call_api("getShares", &[])?;
        Ok(res.shares.map(|w| w.share).unwrap_or_default())
    }

    pub fn create_share(&self, item_id: &str, description: &str) -> Result<Vec<Share>, String> {
        let params = vec![
            ("id", item_id.to_string()),
            ("description", description.to_string()),
        ];
        let res = self.call_api("createShare", &params)?;
        Ok(res.shares.map(|w| w.share).unwrap_or_default())
    }

    pub fn delete_share(&self, share_id: &str) -> Result<(), String> {
        let params = vec![("id", share_id.to_string())];
        let _ = self.call_api("deleteShare", &params)?;
        Ok(())
    }

    /// Fetches synchronized or plain lyrics using OpenSubsonic `getLyricsBySongId` first,
    /// falling back to classic `getLyrics` (with LRC timestamp parsing).
    pub fn get_lyrics_for_song(
        &self,
        song_id: &str,
        artist: &str,
        title: &str,
    ) -> Vec<StructuredLyricLine> {
        // 1. Try OpenSubsonic `getLyricsBySongId`
        if let Ok(res) = self.call_api("getLyricsBySongId", &[("id", song_id.to_string())]) {
            if let Some(list) = res.lyrics_list {
                if let Some(first) = list.structured_lyrics.into_iter().next() {
                    if !first.line.is_empty() {
                        return first.line;
                    }
                }
            }
        }

        // 2. Fallback to classic `getLyrics`
        let params = vec![
            ("artist", artist.to_string()),
            ("title", title.to_string()),
        ];
        if let Ok(res) = self.call_api("getLyrics", &params) {
            if let Some(classic) = res.lyrics {
                if let Some(text) = classic.value {
                    let parsed = parse_lrc_or_plain_lyrics(&text);
                    if !parsed.is_empty() {
                        return parsed;
                    }
                }
            }
        }

        vec![StructuredLyricLine {
            start: Some(0),
            value: "No synchronized or embedded lyrics found for this track.".to_string(),
        }]
    }

    /// Builds a `stream` URL with configurable `format` and `maxBitRate` (and optional `timeOffset`).
    pub fn stream_url(
        &self,
        song_id: &str,
        format: &str,
        max_bit_rate: u32,
        time_offset_secs: u32,
    ) -> String {
        let mut params = vec![
            ("id", song_id.to_string()),
            ("estimateContentLength", "true".to_string()),
        ];
        if !format.is_empty() {
            params.push(("format", format.to_string()));
        }
        if max_bit_rate > 0 {
            params.push(("maxBitRate", max_bit_rate.to_string()));
        }
        if time_offset_secs > 0 {
            params.push(("timeOffset", time_offset_secs.to_string()));
        }
        self.build_url("stream", &params)
    }

    /// Builds a `download` URL with Navidrome's extended `format` and `maxBitRate` parameters
    /// (works for Songs, Albums, Artists, and Playlists in Navidrome).
    pub fn download_url(&self, item_id: &str, format: &str, max_bit_rate: u32) -> String {
        let mut params = vec![("id", item_id.to_string())];
        if !format.is_empty() && format != "raw" {
            params.push(("format", format.to_string()));
        }
        if max_bit_rate > 0 {
            params.push(("maxBitRate", max_bit_rate.to_string()));
        }
        self.build_url("download", &params)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn fetch_stream_bytes(
        &self,
        song_id: &str,
        format: &str,
        max_bit_rate: u32,
    ) -> Result<Vec<u8>, String> {
        let url = self.stream_url(song_id, format, max_bit_rate, 0);
        let resp = self
            .http
            .get(&url)
            .call()
            .map_err(|e| format!("Stream request failed: {}", e))?;
        let mut bytes = Vec::new();
        resp.into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed reading stream bytes: {}", e))?;
        Ok(bytes)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn fetch_download_bytes(
        &self,
        item_id: &str,
        format: &str,
        max_bit_rate: u32,
    ) -> Result<Vec<u8>, String> {
        let url = self.download_url(item_id, format, max_bit_rate);
        let resp = self
            .http
            .get(&url)
            .call()
            .map_err(|e| format!("Download request failed: {}", e))?;
        let mut bytes = Vec::new();
        resp.into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| format!("Failed reading download bytes: {}", e))?;
        Ok(bytes)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn fetch_cover_art(&self, cover_id: &str, size: u32) -> Option<Vec<u8>> {
        if cover_id.is_empty() || !self.is_configured() {
            return None;
        }
        let safe_id: String = cover_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();
        let cache_file = AppConfig::covers_dir().join(format!("{}_{}.img", safe_id, size));
        if cache_file.exists() {
            if let Ok(bytes) = std::fs::read(&cache_file) {
                if !bytes.is_empty() {
                    return Some(bytes);
                }
            }
        }

        let params = vec![
            ("id", cover_id.to_string()),
            ("size", size.to_string()),
        ];
        let url = self.build_url("getCoverArt", &params);
        if let Ok(resp) = self.http.get(&url).call() {
            let mut vec = Vec::new();
            if resp.into_reader().read_to_end(&mut vec).is_ok() && !vec.is_empty() {
                let _ = std::fs::write(&cache_file, &vec);
                return Some(vec);
            }
        }
        None
    }
}

pub fn parse_lrc_or_plain_lyrics(raw: &str) -> Vec<StructuredLyricLine> {
    let mut lines = Vec::new();
    for raw_line in raw.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Check for LRC timestamp `[mm:ss.xx]Text`
        if trimmed.starts_with('[') {
            if let Some(close_idx) = trimmed.find(']') {
                let tag = &trimmed[1..close_idx];
                let text = trimmed[close_idx + 1..].trim();
                if let Some(colon_idx) = tag.find(':') {
                    let min_str = &tag[..colon_idx];
                    let sec_str = &tag[colon_idx + 1..];
                    if let (Ok(mins), Ok(secs)) = (min_str.parse::<i64>(), sec_str.parse::<f64>()) {
                        let start_ms = mins * 60_000 + (secs * 1000.0) as i64;
                        if !text.is_empty() {
                            lines.push(StructuredLyricLine {
                                start: Some(start_ms),
                                value: text.to_string(),
                            });
                            continue;
                        }
                    }
                }
            }
        }
        lines.push(StructuredLyricLine {
            start: None,
            value: trimmed.to_string(),
        });
    }
    lines
}
