use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// --- Generic API response wrapper ---

#[derive(Debug, Default, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(default)]
    pub data: Option<T>,
    #[serde(default)]
    pub error: Option<ApiError>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ApiError {
    pub code: i32,
}

// --- API Info (discovery) ---

#[derive(Debug, Deserialize, Clone)]
pub struct ApiInfo {
    pub path: String,
    #[serde(rename = "minVersion")]
    pub min_version: i32,
    #[serde(rename = "maxVersion")]
    pub max_version: i32,
}

pub type ApiInfoMap = HashMap<String, ApiInfo>;

// --- Auth ---

#[derive(Debug, Default, Deserialize)]
pub struct AuthData {
    pub sid: String,
}

// --- Song ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Song {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub additional: Option<SongAdditional>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SongAdditional {
    #[serde(default)]
    pub song_tag: Option<SongTag>,
    #[serde(default)]
    pub song_audio: Option<SongAudio>,
    #[serde(default)]
    pub song_rating: Option<SongRating>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SongTag {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album_artist: String,
    #[serde(default)]
    pub composer: String,
    #[serde(default)]
    pub genre: String,
    #[serde(default)]
    pub year: i32,
    #[serde(default)]
    pub track: i32,
    #[serde(default)]
    pub disc: i32,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SongAudio {
    #[serde(default)]
    pub duration: i64,
    #[serde(default)]
    pub bitrate: i32,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub container: String,
    #[serde(default)]
    pub frequency: i32,
    #[serde(default)]
    pub channel: i32,
    #[serde(default)]
    pub lossless: bool,
    #[serde(default)]
    pub filesize: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SongRating {
    #[serde(default)]
    pub rating: i32,
}

#[derive(Debug, Default, Deserialize)]
pub struct SongListData {
    #[serde(default)]
    pub songs: Vec<Song>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub offset: i64,
}

// --- Album ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Album {
    pub name: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album_artist: String,
    #[serde(default)]
    pub year: i32,
    #[serde(default)]
    pub display_artist: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct AlbumListData {
    pub albums: Vec<Album>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub offset: i64,
}

// --- Artist ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Artist {
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ArtistListData {
    pub artists: Vec<Artist>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub offset: i64,
}

// --- Playlist ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub library: String,
    #[serde(default)]
    pub songs_count: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PlaylistListData {
    pub playlists: Vec<Playlist>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Default, Deserialize)]
pub struct PlaylistDetailData {
    pub playlist: PlaylistDetail,
}

#[derive(Debug, Default, Deserialize)]
pub struct PlaylistDetail {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub songs: Vec<Song>,
}

// --- Genre ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Genre {
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct GenreListData {
    pub genres: Vec<Genre>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub offset: i64,
}

// --- Composer ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Composer {
    pub name: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct ComposerListData {
    pub composers: Vec<Composer>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub offset: i64,
}

// --- Folder ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Folder {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub is_dir: bool,
}

#[derive(Debug, Default, Deserialize)]
pub struct FolderListData {
    pub items: Vec<Folder>,
    #[serde(default)]
    pub total: i64,
    #[serde(default)]
    pub offset: i64,
}

// --- Pin (Favorites) ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PinItem {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(rename = "type", default)]
    pub item_type: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct PinListData {
    pub items: Vec<PinItem>,
}

// --- Search ---

#[derive(Debug, Default, Deserialize)]
pub struct SearchData {
    #[serde(default)]
    pub songs: Vec<Song>,
    #[serde(default)]
    pub albums: Vec<Album>,
    #[serde(default)]
    pub artists: Vec<Artist>,
}

// --- Lyrics ---

#[derive(Debug, Default, Deserialize)]
pub struct LyricsData {
    #[serde(default)]
    pub lyrics: String,
}

// --- Audio Station Info ---

#[derive(Debug, Default, Deserialize)]
pub struct AudioStationInfo {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub is_manager: bool,
}
