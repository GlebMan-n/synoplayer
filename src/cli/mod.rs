pub mod config;
pub mod library;
pub mod play;
pub mod playlist;
pub mod search;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "synoplayer")]
#[command(about = "CLI audio player for Synology Audio Station")]
#[command(version)]
pub struct Cli {
    /// Skip TUI and show help (by default synoplayer launches TUI)
    #[arg(long)]
    pub no_tui: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Login to Synology NAS
    Login {
        /// Do not save credentials (one-time session)
        #[arg(long)]
        no_save: bool,
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
    Volume { level: u8 },
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
    /// List composers
    Composers,
    /// Browse folders
    Folders { path: Option<String> },
    /// Search music
    Search { keyword: String },
    /// Manage playlists
    Playlist {
        #[command(subcommand)]
        action: PlaylistAction,
    },
    /// List all playlists
    Playlists,
    /// Rate a song (1-5)
    Rate { song_id: String, rating: i32 },
    /// Add to favorites
    Favorite { song_id: String },
    /// Remove from favorites
    Unfavorite { song_id: String },
    /// List favorites
    Favorites,
    /// Show lyrics
    Lyrics { song_id: Option<String> },
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
    /// Download a track to local file
    Download {
        /// Song ID or name
        song_id: String,
        /// Output file path (default: current dir, auto-named)
        #[arg(long, short)]
        output: Option<String>,
    },
    /// View or manage playback history
    History {
        #[command(subcommand)]
        action: Option<HistoryAction>,
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
    /// Launch interactive TUI player
    Tui,
    /// Generate shell completion scripts
    Completion {
        /// Shell type
        #[arg(value_enum)]
        shell: clap_complete::Shell,
        /// Auto-install to standard location
        #[arg(long)]
        install: bool,
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
    /// Rename a playlist
    Rename { name: String, new_name: String },
    /// Add a song to a playlist
    Add { playlist: String, song_id: String },
    /// Remove a song from a playlist
    Remove { playlist: String, song_id: String },
    /// Play entire playlist sequentially
    Play {
        name: String,
        /// Start from track N (1-based)
        #[arg(long, default_value_t = 1)]
        from: usize,
        /// Shuffle playback order
        #[arg(long)]
        shuffle: bool,
        /// Repeat mode: off, one, all
        #[arg(long, default_value = "off")]
        repeat: String,
    },
    /// Import a .m3u playlist file from NAS filesystem
    Import {
        /// Path to .m3u file on NAS (e.g. /volume1/homes/user/music/playlists/My.m3u)
        path: String,
        /// Playlist name (defaults to filename without extension)
        #[arg(long)]
        name: Option<String>,
    },
    /// Create a smart playlist with filter rules
    Smart {
        /// Playlist name
        name: String,
        /// Filter by genre
        #[arg(long)]
        genre: Option<String>,
        /// Filter by artist
        #[arg(long)]
        artist: Option<String>,
        /// Minimum rating (0-5)
        #[arg(long)]
        min_rating: Option<i32>,
        /// Filter by year (e.g. 2020)
        #[arg(long)]
        year: Option<i32>,
        /// Maximum number of songs
        #[arg(long, default_value_t = 100)]
        limit: i64,
    },
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
pub enum HistoryAction {
    /// Clear playback history
    Clear,
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
