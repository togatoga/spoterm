extern crate crossbeam;
extern crate rspotify;

use rspotify::spotify;
use std::thread;

pub enum SpotifyAPIEvent {
    Pause(Option<String>),
    CurrentPlayBack,
    CurrentUserRecentlyPlayed,
    Device,
    StartPlayBack((Option<String>, Option<Vec<String>>)),
}

pub enum SpotifyAPIResult {
    CurrentPlayBack(Option<spotify::model::context::FullPlayingContext>),
    CurrentUserPlayingTrack(Option<spotify::model::playing::Playing>),
    CurrentUserRecentlyPlayed(Vec<spotify::model::playing::PlayHistory>),
    Device(Vec<spotify::model::device::Device>),
}

pub struct SpotifyService {
    pub client: spotify::client::Spotify,
    pub api_result_tx: Option<crossbeam::channel::Sender<SpotifyAPIResult>>,
    pub api_event_tx: crossbeam::channel::Sender<SpotifyAPIEvent>,
    pub api_event_rx: crossbeam::channel::Receiver<SpotifyAPIEvent>,
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

impl SpotifyService {
    pub fn new(client_id: String, client_secret: String) -> SpotifyService {
        let spoterm_cache = dirs::home_dir()
            .expect("can not find home directory")
            .join(".spoterm")
            .join(".spotify_token_cache.json");
        let mut oauth = spotify::oauth2::SpotifyOAuth::default()
            .scope(&SCOPES.join(" "))
            .client_id(&client_id)
            .client_secret(&client_secret)
            .redirect_uri("http://localhost:8888/callback")
            .cache_path(spoterm_cache)
            .build();
        let token_info = spotify::util::get_token(&mut oauth).expect("Auth failed");
        let client_credential = spotify::oauth2::SpotifyClientCredentials::default()
            .token_info(token_info)
            .build();
        let spotify = spotify::client::Spotify::default()
            .client_credentials_manager(client_credential)
            .build();

        let (tx, rx) = crossbeam::channel::unbounded();

        SpotifyService {
            client: spotify,
            api_result_tx: None,
            api_event_tx: tx,
            api_event_rx: rx,
        }
    }
    pub fn api_result_tx(mut self, tx: crossbeam::channel::Sender<SpotifyAPIResult>) -> Self {
        self.api_result_tx = Some(tx);
        self
    }
    pub fn run(self) -> Result<(), failure::Error> {
        let rx = self.api_event_rx.clone();
        thread::spawn(move || loop {
            match rx.recv().unwrap() {
                SpotifyAPIEvent::Pause(device_id) => {
                    self.pause_playback(device_id);
                }
                SpotifyAPIEvent::Device => {
                    self.fetch_device();
                }
                SpotifyAPIEvent::CurrentUserRecentlyPlayed => {
                    self.fetch_current_user_recently_played();
                }
                SpotifyAPIEvent::StartPlayBack((device_id, uris)) => {
                    self.fetch_start_playback(device_id, uris);
                }
                SpotifyAPIEvent::CurrentPlayBack => {
                    self.fetch_current_playback();
                }
                _ => {}
            }
        });
        Ok(())
    }
    fn fetch_start_playback(
        &self,
        device_id: Option<String>,
        uris: Option<Vec<String>>,
    ) -> Result<(), failure::Error> {
        self.client.start_playback(device_id, None, uris, None)
    }
    fn fetch_current_user_recently_played(&self) -> Result<(), failure::Error> {
        let items = self.client.clone().current_user_recently_played(50)?.items;
        self.api_result_tx
            .clone()
            .unwrap()
            .send(SpotifyAPIResult::CurrentUserRecentlyPlayed(items))?;
        Ok(())
    }
    fn fetch_current_playback(&self) -> Result<(), failure::Error> {
        let current_playback = self.client.current_playback(None)?;
        self.api_result_tx
            .clone()
            .unwrap()
            .send(SpotifyAPIResult::CurrentPlayBack(current_playback))?;
        Ok(())
    }
    fn fetch_current_user_playing_track(&self) -> Result<(), failure::Error> {
        let playing_track = self.client.current_user_playing_track()?;
        self.api_result_tx
            .clone()
            .unwrap()
            .send(SpotifyAPIResult::CurrentUserPlayingTrack(playing_track))?;
        Ok(())
    }
    fn fetch_device(&self) -> Result<(), failure::Error> {
        let devices = self.client.device()?.devices;
        self.api_result_tx
            .clone()
            .unwrap()
            .send(SpotifyAPIResult::Device(devices))?;
        Ok(())
    }
    fn pause_playback(&self, device_id: Option<String>) -> Result<(), failure::Error> {
        self.client.pause_playback(device_id);
        Ok(())
    }
}
