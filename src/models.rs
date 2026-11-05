use serde::{Deserialize, Deserializer, Serialize};

/// Helper deserializer that accepts either a JSON string or integer/number and converts it to a String.
/// Navidrome always uses string IDs (MD5/UUID), while legacy Subsonic servers may return integer IDs.
pub fn deserialize_string_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt {
        Str(String),
        Int(i64),
        UInt(u64),
    }

    match StringOrInt::deserialize(deserializer)? {
        StringOrInt::Str(s) => Ok(s),
        StringOrInt::Int(i) => Ok(i.to_string()),
        StringOrInt::UInt(u) => Ok(u.to_string()),
    }
}

pub fn deserialize_opt_string_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OptStringOrInt {
        Str(String),
        Int(i64),
        UInt(u64),
        Null,
    }

    match Option::<OptStringOrInt>::deserialize(deserializer)? {
        Some(OptStringOrInt::Str(s)) => Ok(Some(s)),
        Some(OptStringOrInt::Int(i)) => Ok(Some(i.to_string())),
        Some(OptStringOrInt::UInt(u)) => Ok(Some(u.to_string())),
        Some(OptStringOrInt::Null) | None => Ok(None),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubsonicEnvelope {
    #[serde(rename = "subsonic-response")]
    pub response: SubsonicResponse,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubsonicResponse {
    pub status: String,
    pub version: String,
    #[serde(default)]
    pub r#type: Option<String>,
    #[serde(default)]
    pub server_version: Option<String>,
    #[serde(default)]
    pub open_subsonic: Option<bool>,
    #[serde(default)]
    pub error: Option<SubsonicError>,

    #[serde(default)]
    pub music_folders: Option<MusicFoldersWrapper>,
    #[serde(default)]
    pub album_list2: Option<AlbumListWrapper>,
    #[serde(default)]
    pub album: Option<Album>,
    #[serde(default)]
    pub album_info2: Option<AlbumInfo>,
    #[serde(default)]
    pub artists: Option<ArtistsWrapper>,
    #[serde(default)]
    pub artist: Option<ArtistDetail>,
    #[serde(default)]
    pub artist_info2: Option<ArtistInfo>,
    #[serde(default)]
    pub song: Option<Song>,
    #[serde(default)]
    pub random_songs: Option<SongListWrapper>,
    #[serde(default)]
    pub songs_by_genre: Option<SongListWrapper>,
    #[serde(default)]
    pub top_songs: Option<SongListWrapper>,
    #[serde(default)]
    pub similar_songs2: Option<SongListWrapper>,
    #[serde(default)]
    pub starred2: Option<StarredWrapper>,
    #[serde(default)]
    pub search_result3: Option<SearchResult3>,
    #[serde(default)]
    pub genres: Option<GenresWrapper>,
    #[serde(default)]
    pub playlists: Option<PlaylistsWrapper>,
    #[serde(default)]
    pub playlist: Option<Playlist>,
    #[serde(default)]
    pub lyrics: Option<LyricsClassic>,
    #[serde(default)]
    pub lyrics_list: Option<LyricsListWrapper>,
    #[serde(default)]
    pub play_queue: Option<PlayQueue>,
    #[serde(default)]
    pub scan_status: Option<ScanStatus>,
    #[serde(default)]
    pub shares: Option<SharesWrapper>,
    #[serde(default)]
    pub open_subsonic_extensions: Option<Vec<OpenSubsonicExtension>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubsonicError {
    pub code: i32,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicFoldersWrapper {
    #[serde(default, rename = "musicFolder")]
    pub music_folder: Vec<MusicFolder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicFolder {
    #[serde(deserialize_with = "deserialize_string_id")]
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlbumListWrapper {
    #[serde(default)]
    pub album: Vec<Album>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    #[serde(deserialize_with = "deserialize_string_id")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub display_artist: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_id")]
    pub artist_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_id")]
    pub cover_art: Option<String>,
    #[serde(default)]
    pub song_count: u32,
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub year: Option<u32>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub starred: Option<String>,
    #[serde(default)]
    pub user_rating: Option<u32>,
    #[serde(default)]
    pub song: Vec<Song>,
}

impl Album {
    pub fn display_title(&self) -> String {
        if !self.name.is_empty() {
            self.name.clone()
        } else {
            self.title.clone().unwrap_or_else(|| "Unknown Album".to_string())
        }
    }

    pub fn display_artist_name(&self) -> String {
        self.display_artist
            .clone()
            .or_else(|| self.artist.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string())
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlbumInfo {
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub music_brainz_id: Option<String>,
    #[serde(default)]
    pub last_fm_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ArtistsWrapper {
    #[serde(default)]
    pub index: Vec<ArtistIndex>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ArtistIndex {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub artist: Vec<Artist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    #[serde(deserialize_with = "deserialize_string_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub album_count: u32,
    #[serde(default)]
    pub starred: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_id")]
    pub cover_art: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDetail {
    #[serde(deserialize_with = "deserialize_string_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub album_count: u32,
    #[serde(default)]
    pub starred: Option<String>,
    #[serde(default)]
    pub album: Vec<Album>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInfo {
    #[serde(default)]
    pub biography: Option<String>,
    #[serde(default)]
    pub music_brainz_id: Option<String>,
    #[serde(default)]
    pub last_fm_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ReplayGain {
    #[serde(default)]
    pub track_gain: Option<f32>,
    #[serde(default)]
    pub album_gain: Option<f32>,
    #[serde(default)]
    pub track_peak: Option<f32>,
    #[serde(default)]
    pub album_peak: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    #[serde(deserialize_with = "deserialize_string_id")]
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_opt_string_id")]
    pub parent: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub album: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_id")]
    pub album_id: Option<String>,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub display_artist: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_id")]
    pub artist_id: Option<String>,
    #[serde(default)]
    pub track: Option<u32>,
    #[serde(default)]
    pub disc_number: Option<u32>,
    #[serde(default)]
    pub year: Option<u32>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default, deserialize_with = "deserialize_opt_string_id")]
    pub cover_art: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub suffix: Option<String>,
    #[serde(default)]
    pub transcoded_content_type: Option<String>,
    #[serde(default)]
    pub transcoded_suffix: Option<String>,
    #[serde(default)]
    pub duration: Option<u32>,
    #[serde(default)]
    pub bit_rate: Option<u32>,
    #[serde(default)]
    pub bit_depth: Option<u32>,
    #[serde(default)]
    pub sampling_rate: Option<u32>,
    #[serde(default)]
    pub channel_count: Option<u32>,
    #[serde(default)]
    pub starred: Option<String>,
    #[serde(default)]
    pub user_rating: Option<u32>,
    #[serde(default)]
    pub play_count: Option<u64>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub replay_gain: Option<ReplayGain>,
}

impl Song {
    pub fn display_artist_name(&self) -> String {
        self.display_artist
            .clone()
            .or_else(|| self.artist.clone())
            .unwrap_or_else(|| "Unknown Artist".to_string())
    }

    pub fn display_album_name(&self) -> String {
        self.album.clone().unwrap_or_else(|| "Single".to_string())
    }

    pub fn quality_badge(&self) -> String {
        let ext = self
            .suffix
            .as_deref()
            .unwrap_or("AUDIO")
            .to_ascii_uppercase();
        if let (Some(depth), Some(rate)) = (self.bit_depth, self.sampling_rate) {
            if depth > 0 && rate > 0 {
                let khz = (rate as f32) / 1000.0;
                if (khz - khz.round()).abs() < 0.05 {
                    return format!("{} {}/{:.0}", ext, depth, khz);
                } else {
                    return format!("{} {}/{:.1}", ext, depth, khz);
                }
            }
        }
        if let Some(br) = self.bit_rate {
            if br > 0 {
                return format!("{} {}k", ext, br);
            }
        }
        ext
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SongListWrapper {
    #[serde(default)]
    pub song: Vec<Song>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct StarredWrapper {
    #[serde(default)]
    pub artist: Vec<Artist>,
    #[serde(default)]
    pub album: Vec<Album>,
    #[serde(default)]
    pub song: Vec<Song>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SearchResult3 {
    #[serde(default)]
    pub artist: Vec<Artist>,
    #[serde(default)]
    pub album: Vec<Album>,
    #[serde(default)]
    pub song: Vec<Song>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GenresWrapper {
    #[serde(default)]
    pub genre: Vec<Genre>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub song_count: u32,
    #[serde(default)]
    pub album_count: u32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PlaylistsWrapper {
    #[serde(default)]
    pub playlist: Vec<Playlist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    #[serde(deserialize_with = "deserialize_string_id")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub public: Option<bool>,
    #[serde(default)]
    pub song_count: u32,
    #[serde(default)]
    pub duration: u32,
    #[serde(default)]
    pub entry: Vec<Song>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LyricsClassic {
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LyricsListWrapper {
    #[serde(default)]
    pub structured_lyrics: Vec<StructuredLyrics>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StructuredLyrics {
    #[serde(default)]
    pub display_artist: Option<String>,
    #[serde(default)]
    pub display_title: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub synced: bool,
    #[serde(default)]
    pub line: Vec<StructuredLyricLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StructuredLyricLine {
    #[serde(default)]
    pub start: Option<i64>,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayQueue {
    /// Note: In Navidrome, `current` is a string ID, not an integer.
    #[serde(default, deserialize_with = "deserialize_opt_string_id")]
    pub current: Option<String>,
    #[serde(default)]
    pub position: Option<u64>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub entry: Vec<Song>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatus {
    #[serde(default)]
    pub scanning: bool,
    #[serde(default)]
    pub count: Option<u64>,
    #[serde(default)]
    pub last_scan: Option<String>,
    #[serde(default)]
    pub folder_count: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SharesWrapper {
    #[serde(default)]
    pub share: Vec<Share>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Share {
    #[serde(deserialize_with = "deserialize_string_id")]
    pub id: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub visit_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenSubsonicExtension {
    pub name: String,
    #[serde(default)]
    pub versions: Vec<u32>,
}

pub fn format_duration(seconds: u32) -> String {
    let mins = seconds / 60;
    let secs = seconds % 60;
    if mins >= 60 {
        let hrs = mins / 60;
        let rem_mins = mins % 60;
        format!("{}:{:02}:{:02}", hrs, rem_mins, secs)
    } else {
        format!("{}:{:02}", mins, secs)
    }
}

/// Decodes JPEG/PNG/WebP bytes from Navidrome `getCoverArt` into a `slint::Image`.
pub fn decode_cover_image(bytes: &[u8]) -> Option<slint::Image> {
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(img.as_raw(), w, h);
    Some(slint::Image::from_rgba8(buffer))
}
