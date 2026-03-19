//! Tests for parsing Synology API JSON responses into typed structs.
//! These tests use fixture files and require no network access.

use synoplayer::api::types::*;

#[test]
fn parse_api_info_response() {
    let json = include_str!("fixtures/api_info_response.json");
    let response: ApiResponse<ApiInfoMap> = serde_json::from_str(json).unwrap();

    assert!(response.success);
    let data = response.data.unwrap();
    assert!(data.contains_key("SYNO.API.Auth"));
    assert!(data.contains_key("SYNO.AudioStation.Song"));
    assert!(data.contains_key("SYNO.AudioStation.Stream"));

    let song_info = &data["SYNO.AudioStation.Song"];
    assert_eq!(song_info.path, "AudioStation/song.cgi");
    assert_eq!(song_info.min_version, 1);
    assert_eq!(song_info.max_version, 3);
}

#[test]
fn parse_auth_login_success() {
    let json = include_str!("fixtures/auth_login_success.json");
    let response: ApiResponse<AuthData> = serde_json::from_str(json).unwrap();

    assert!(response.success);
    let data = response.data.unwrap();
    assert!(!data.sid.is_empty());
}

#[test]
fn parse_auth_login_wrong_password() {
    let json = include_str!("fixtures/auth_login_wrong_password.json");
    let response: ApiResponse<AuthData> = serde_json::from_str(json).unwrap();

    assert!(!response.success);
    assert_eq!(response.error.unwrap().code, 400);
}

#[test]
fn parse_auth_login_2fa() {
    let json = include_str!("fixtures/auth_login_2fa_required.json");
    let response: ApiResponse<AuthData> = serde_json::from_str(json).unwrap();

    assert!(!response.success);
    assert_eq!(response.error.unwrap().code, 403);
}

#[test]
fn parse_error_session_expired() {
    let json = include_str!("fixtures/error_session_expired.json");
    let response: ApiResponse<()> = serde_json::from_str(json).unwrap();

    assert!(!response.success);
    assert_eq!(response.error.unwrap().code, 106);
}

#[test]
fn parse_error_no_permission() {
    let json = include_str!("fixtures/error_no_permission.json");
    let response: ApiResponse<()> = serde_json::from_str(json).unwrap();

    assert!(!response.success);
    assert_eq!(response.error.unwrap().code, 105);
}

#[test]
fn parse_song_list_response() {
    let json = include_str!("fixtures/song_list_response.json");
    let response: ApiResponse<SongListData> = serde_json::from_str(json).unwrap();

    assert!(response.success);
    let data = response.data.unwrap();
    assert_eq!(data.total, 1234);
    assert_eq!(data.offset, 0);
    assert_eq!(data.songs.len(), 2);

    let song = &data.songs[0];
    assert_eq!(song.id, "music_12345");
    assert_eq!(song.title, "Comfortably Numb");

    let tag = song.additional.as_ref().unwrap().song_tag.as_ref().unwrap();
    assert_eq!(tag.artist, "Pink Floyd");
    assert_eq!(tag.album, "The Wall");
    assert_eq!(tag.year, 1979);
    assert_eq!(tag.track, 6);
    assert_eq!(tag.disc, 2);

    let audio = song
        .additional
        .as_ref()
        .unwrap()
        .song_audio
        .as_ref()
        .unwrap();
    assert_eq!(audio.duration, 382);
    assert_eq!(audio.codec, "flac");
    assert!(audio.lossless);
    assert_eq!(audio.frequency, 44100);

    let rating = song
        .additional
        .as_ref()
        .unwrap()
        .song_rating
        .as_ref()
        .unwrap();
    assert_eq!(rating.rating, 5);
}

#[test]
fn parse_song_rating_range() {
    let json = include_str!("fixtures/song_list_response.json");
    let response: ApiResponse<SongListData> = serde_json::from_str(json).unwrap();
    let data = response.data.unwrap();

    for song in &data.songs {
        if let Some(additional) = &song.additional {
            if let Some(rating) = &additional.song_rating {
                assert!(rating.rating >= 0 && rating.rating <= 5);
            }
        }
    }
}

#[test]
fn parse_album_list_response() {
    let json = include_str!("fixtures/album_list_response.json");
    let response: ApiResponse<AlbumListData> = serde_json::from_str(json).unwrap();

    assert!(response.success);
    let data = response.data.unwrap();
    assert_eq!(data.total, 42);
    assert_eq!(data.albums.len(), 2);
    assert_eq!(data.albums[0].name, "The Dark Side of the Moon");
    assert_eq!(data.albums[0].year, 1973);
}

#[test]
fn parse_playlist_list_response() {
    let json = include_str!("fixtures/playlist_list_response.json");
    let response: ApiResponse<PlaylistListData> = serde_json::from_str(json).unwrap();

    assert!(response.success);
    let data = response.data.unwrap();
    assert_eq!(data.total, 3);
    assert_eq!(data.playlists.len(), 3);

    let personal = &data.playlists[0];
    assert_eq!(personal.name, "My Favorites");
    assert_eq!(personal.library, "personal");

    let shared = &data.playlists[2];
    assert_eq!(shared.library, "shared");
}

#[test]
fn parse_pin_list_response() {
    let json = include_str!("fixtures/pin_list_response.json");
    let response: ApiResponse<PinListData> = serde_json::from_str(json).unwrap();

    assert!(response.success);
    let data = response.data.unwrap();
    assert_eq!(data.total, 2);
    assert_eq!(data.items.len(), 2);

    // First pin: song with empty name (not renamed)
    assert_eq!(data.items[0].id, "music_12345");
    assert_eq!(data.items[0].title, "Comfortably Numb");
    assert_eq!(data.items[0].name, "");
    assert_eq!(data.items[0].item_type, "song");

    // Second pin: album with custom name (renamed)
    assert_eq!(data.items[1].id, "album_42");
    assert_eq!(data.items[1].title, "The Dark Side of the Moon");
    assert_eq!(data.items[1].name, "My Fav Album");
    assert_eq!(data.items[1].item_type, "album");
}

#[test]
fn pin_item_display_name_prefers_name_over_title() {
    let json = include_str!("fixtures/pin_list_response.json");
    let response: ApiResponse<PinListData> = serde_json::from_str(json).unwrap();
    let data = response.data.unwrap();

    // When name is empty, title should be used
    let item0 = &data.items[0];
    let display0 = if item0.name.is_empty() {
        &item0.title
    } else {
        &item0.name
    };
    assert_eq!(display0, "Comfortably Numb");

    // When name is set (renamed), name should be used
    let item1 = &data.items[1];
    let display1 = if item1.name.is_empty() {
        &item1.title
    } else {
        &item1.name
    };
    assert_eq!(display1, "My Fav Album");
}

#[test]
fn error_code_mapping() {
    use synoplayer::error::SynoError;

    let err = SynoError::from_api_code(106);
    assert!(err.is_session_expired());

    let err = SynoError::from_api_code(119);
    assert!(err.is_session_expired());

    let err = SynoError::from_api_code(400);
    assert!(matches!(err, SynoError::InvalidCredentials));

    let err = SynoError::from_api_code(403);
    assert!(matches!(err, SynoError::TwoFactorRequired));

    let err = SynoError::from_api_code(105);
    assert!(matches!(err, SynoError::Api { code: 105, .. }));
}
