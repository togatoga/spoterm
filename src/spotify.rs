extern crate failure;
extern crate rspotify;

use super::ui;
use rspotify::spotify::client::Spotify;
use rspotify::spotify::model::device::Device;
use rspotify::spotify::model::playing::PlayHistory;
use rspotify::spotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth};
use rspotify::spotify::util::get_token;

pub struct SpotifyClient {
    pub spotify: Spotify,
    pub selected_device: Option<Device>,
    pub recent_played: ui::RecentPlayed,
}
//Authorization Scopes
//https://developer.spotify.com/documentation/general/guides/scopes/
const SCOPES: [&'static str; 18] = [
    //Listening History
    "user-top-read",
    "user-read-recently-played",
    //Spotify Connect
    "user-read-playback-state",
    "user-read-currently-playing",
    "user-modify-playback-state",
    //Library
    "user-library-modify",
    "user-library-read",
    //Playback
    "streaming",
    "app-remote-control",
    "user-read-private",
    "user-read-birthdate",
    "user-read-email",
    //Follow
    "user-follow-modify",
    "user-follow-read",
    //PlayLists
    "playlist-modify-public",
    "playlist-read-collaborative",
    "playlist-read-private",
    "playlist-modify-private",
];

impl SpotifyClient {
    pub fn new(client_id: String, client_secret: String) -> SpotifyClient {
        let spoterm_cache = dirs::home_dir()
            .expect("can not find home directory")
            .join(".spoterm")
            .join(".spotify_token_cache.json");
        let mut oauth = SpotifyOAuth::default()
            .scope(&SCOPES.join(" "))
            .client_id(&client_id)
            .client_secret(&client_secret)
            .redirect_uri("http://localhost:8888/callback")
            .cache_path(spoterm_cache)
            .build();
        let token_info = get_token(&mut oauth).expect("Auth failed");
        let client_credential = SpotifyClientCredentials::default()
            .token_info(token_info)
            .build();
        let spotify: Spotify = Spotify::default()
            .client_credentials_manager(client_credential)
            .build();
        SpotifyClient {
            spotify: spotify,
            selected_device: None,
            recent_played: ui::RecentPlayed::new(),
        }
    }
    pub fn fetch_device(&mut self) -> Result<(), failure::Error> {
        match self.spotify.device() {
            Ok(device_pay_load) => {
                for device in device_pay_load.devices {
                    //hardcode X(
                    if device.name == "sheringham" {
                        self.selected_device = Some(device);
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                return Err(e);
            }
        }
        Ok(())
    }
    pub fn fetch_recent_play_history(&mut self) -> Result<(), failure::Error> {
        match self.spotify.clone().current_user_recently_played(50) {
            Ok(play_history) => {
                self.recent_played.recent_play_histories = Some(play_history.items);
            }
            Err(e) => {
                return Err(e);
            }
        };
        Ok(())
    }
}
