extern crate failure;
extern crate hostname;
extern crate itertools;
extern crate rspotify;
extern crate unicode_width;
use itertools::Itertools;

use self::rspotify::spotify::model::playing::Playing;
use super::ui;
use crate::spotify::{SpotifyAPIEvent, SpotifyAPIResult, SpotifyService};
use crate::ui::{Contents, RecentPlayed, UI};

use self::rspotify::spotify::model::context::FullPlayingContext;
use rspotify::spotify::client::Spotify;
use rspotify::spotify::model::device::Device;
use rspotify::spotify::model::playing::PlayHistory;
use rspotify::spotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth};
use rspotify::spotify::util::get_token;
use std::thread;

#[derive(Clone, Debug)]
pub struct SpotifyData {
    pub devices: Option<Vec<Device>>,
    pub recent_play_histories: Option<Vec<PlayHistory>>,
    pub current_playback: Option<FullPlayingContext>,
    pub selected_device: Option<Device>,
}

impl SpotifyData {
    pub fn new() -> SpotifyData {
        SpotifyData {
            devices: None,
            recent_play_histories: None,
            current_playback: None,
            selected_device: None,
        }
    }
}

pub struct SpotermClient {
    pub tx: crossbeam::channel::Sender<SpotifyAPIEvent>,
    pub rx: crossbeam::channel::Receiver<SpotifyAPIResult>,
    //data from api
    pub spotify_data: SpotifyData,
    //data for ui
    pub menu_tabs: Vec<String>,
    pub selected_menu_tab_id: usize,
    pub contents: Contents,
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

impl SpotermClient {
    pub fn new(client_id: String, client_secret: String) -> SpotermClient {
        let (tx, rx) = crossbeam::channel::unbounded();
        let spotify = SpotifyService::new(client_id, client_secret).api_result_tx(tx.clone());

        let api_event_tx = spotify.api_event_tx.clone();
        let contents = Contents::new().ui(RecentPlayed::new(api_event_tx.clone()));
        spotify.run();
        SpotermClient {
            tx: api_event_tx.clone(),
            rx: rx.clone(),
            spotify_data: SpotifyData::new(),
            menu_tabs: vec![
                "Recently Played".to_string(),
                "Albums".to_string(),
                "Artists".to_string(),
            ],
            selected_menu_tab_id: 0,
            contents: contents,
        }
    }
    pub fn fetch_api_result(&mut self) {
        for result in self.rx.try_recv() {
            match result {
                SpotifyAPIResult::Device(devices) => {
                    self.spotify_data.devices = Some(devices);
                }
                SpotifyAPIResult::CurrentUserRecentlyPlayed(recent_play_histories) => {
                    self.spotify_data.recent_play_histories = Some(recent_play_histories);
                }
                SpotifyAPIResult::CurrentPlayBack(current_playback) => {
                    self.spotify_data.current_playback = current_playback;
                }
                _ => {}
            }
        }
    }
    pub fn move_to_next_menu_tab(&mut self) {
        if self.selected_menu_tab_id + 1 < self.menu_tabs.len() {
            self.selected_menu_tab_id += 1;
        } else {
            self.selected_menu_tab_id = 0;
        }
    }
    pub fn move_to_previous_menu_tab(&mut self) {
        if self.selected_menu_tab_id > 0 {
            self.selected_menu_tab_id -= 1;
        } else {
            self.selected_menu_tab_id = self.menu_tabs.len() - 1;
        }
    }
    pub fn request_current_playback(&self) {
        self.tx.send(SpotifyAPIEvent::CurrentPlayBack).unwrap();
    }
    pub fn request_current_user_recently_played(&self) {
        self.tx
            .send(SpotifyAPIEvent::CurrentUserRecentlyPlayed)
            .unwrap();
    }
    pub fn request_device(&self) {
        self.tx.send(SpotifyAPIEvent::Device).unwrap();
    }

    pub fn pause(&self) {
        if self.spotify_data.selected_device.is_none() {
            return;
        }
        let device_id = self
            .spotify_data
            .selected_device
            .as_ref()
            .unwrap()
            .id
            .clone();
        if let Some(current_playback) = self.spotify_data.current_playback.as_ref() {
            //pause
            if current_playback.is_playing {
                self.tx
                    .send(SpotifyAPIEvent::Pause(Some(device_id)))
                    .unwrap();
            } else {
                //unpause
                self.tx
                    .send(SpotifyAPIEvent::StartPlayBack((Some(device_id), None)))
                    .unwrap();
            }
        }

        //self.tx.send(SpotifyAPIEvent::Pause(Some(device_id))).unwrap();
    }

    pub fn set_selected_device(&mut self) -> Result<(), failure::Error> {
        //skip
        if self.spotify_data.selected_device.is_some() || self.spotify_data.devices.is_none() {
            return Ok(());
        }
        let local_hostname = hostname::get_hostname().expect("can not get hostname");
        let devices = self.spotify_data.devices.clone().unwrap();
        for device in devices {
            if device.name == local_hostname {
                self.spotify_data.selected_device = Some(device);
                return Ok(());
            }
        }
        Ok(())
    }
}