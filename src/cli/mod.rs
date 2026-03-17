pub mod play;
pub mod playlist;
pub mod library;
pub mod search;
pub mod config;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "synoplayer")]
#[command(about = "CLI audio player for Synology Audio Station")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Login to Synology NAS
    Login {
        /// Save credentials for auto-login
        #[arg(long, default_value_t = true)]
        save: bool,
    },
    /// Logout from Synology NAS
    Logout,
    /// Clear saved credentials
    #[command(name = "credentials")]
    Credentials {
        #[command(subcommand)]
        action: CredentialAction,
    },
    /// Play a song
    Play {
        /// Song ID or name
        target: String,
    },
    /// Pause playback
    Pause,
    /// Resume playback
    Resume,
    /// Stop playback
    Stop,
    /// Next track
    Next,
    /// Previous track
    Prev,
    /// Set volume (0-100)
    Volume {
        level: u8,
    },
    /// Show current track
    Now,
    /// Show play queue
    Queue,
    /// List songs
    Songs {
        #[arg(long)]
        album: Option<String>,
        #[arg(long)]
        artist: Option<String>,
        #[arg(long)]
        genre: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    /// List albums
    Albums {
        #[arg(long)]
        artist: Option<String>,
    },
    /// List artists
    Artists,
    /// List genres
    Genres,
    /// Browse folders
    Folders {
        path: Option<String>,
    },
    /// Search music
    Search {
        keyword: String,
    },
    /// Manage playlists
    Playlist {
        #[command(subcommand)]
        action: PlaylistAction,
    },
    /// List all playlists
    Playlists,
    /// Rate a song (1-5)
    Rate {
        song_id: String,
        rating: i32,
    },
    /// Add to favorites
    Favorite {
        song_id: String,
    },
    /// Remove from favorites
    Unfavorite {
        song_id: String,
    },
    /// List favorites
    Favorites,
    /// Show lyrics
    Lyrics {
        song_id: Option<String>,
    },
    /// Internet radio
    Radio {
        #[command(subcommand)]
        action: RadioAction,
    },
    /// Set shuffle mode
    Shuffle {
        #[arg(default_value = "on")]
        mode: String,
    },
    /// Set repeat mode
    Repeat {
        #[arg(default_value = "off")]
        mode: String,
    },
    /// Cache management
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
pub enum CredentialAction {
    /// Clear saved credentials
    Clear,
}

#[derive(Subcommand)]
pub enum PlaylistAction {
    /// Show playlist contents
    Show { name: String },
    /// Create a new playlist
    Create { name: String },
    /// Delete a playlist
    Delete { name: String },
    /// Add a song to a playlist
    Add { playlist: String, song_id: String },
    /// Remove a song from a playlist
    Remove { playlist: String, song_id: String },
    /// Play a playlist
    Play { name: String },
}

#[derive(Subcommand)]
pub enum RadioAction {
    /// List radio stations
    List,
    /// Play a radio station
    Play { station: String },
    /// Add a radio station
    Add { name: String, url: String },
}

#[derive(Subcommand)]
pub enum CacheAction {
    /// Show cache status
    Status,
    /// Clear cache
    Clear {
        #[arg(long)]
        older: Option<String>,
    },
    /// Preload a playlist
    Preload { playlist: String },
    /// List cached tracks
    List,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Set server address
    SetServer { host: String },
    /// Set server port
    SetPort { port: u16 },
    /// Show current config
    Show,
}
