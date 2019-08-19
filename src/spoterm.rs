extern crate failure;
extern crate hostname;
extern crate itertools;
extern crate rspotify;
extern crate unicode_width;
use itertools::Itertools;

use self::rspotify::spotify::model::playing::Playing;
use super::ui;
use crate::spotify::{SpotifyAPIEvent, SpotifyAPIResult, SpotifyService};
use crate::ui::{Contents, LikedSongs, RecentPlayed, UI};

use self::rspotify::spotify::model::context::FullPlayingContext;
use self::rspotify::spotify::model::page::Page;
use self::rspotify::spotify::model::track::SavedTrack;
use rspotify::spotify::client::Spotify;
use rspotify::spotify::model::device::Device;
use rspotify::spotify::model::playing::PlayHistory;
use rspotify::spotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth};
use rspotify::spotify::util::get_token;
use rspotify::spotify::senum::RepeatState;
use std::cmp;
use std::thread;
use tui::style::{Color, Style};
use tui::widgets::Text;

#[derive(Clone, Debug)]
pub struct SpotifyData {
    pub devices: Option<Vec<Device>>,
    pub page_saved_tracks: Vec<Page<SavedTrack>>,
    pub recent_play_histories: Option<Vec<PlayHistory>>,
    pub current_playback: Option<FullPlayingContext>,
    pub selected_device: Option<Device>,
}

impl SpotifyData {
    pub fn new() -> SpotifyData {
        SpotifyData {
            devices: None,
            page_saved_tracks: Vec::new(),
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
        let contents = Contents::new()
            .ui(RecentPlayed::new(api_event_tx.clone()))
            .ui(LikedSongs::new(api_event_tx.clone()));
        spotify.run();
        SpotermClient {
            tx: api_event_tx.clone(),
            rx: rx.clone(),
            spotify_data: SpotifyData::new(),
            menu_tabs: vec![
                "📝 Recently Played 📝".to_string(),
                "❤ Liked Songs ❤".to_string(),
                //"Artists".to_string(),
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
                SpotifyAPIResult::CurrentUserSavedTracks(page_saved_tracks) => {
                    if page_saved_tracks.next.is_some() {
                        self.tx.send(SpotifyAPIEvent::CurrentUserSavedTracks(Some(page_saved_tracks.offset + page_saved_tracks.limit))).unwrap();
                    }
                    self.spotify_data.page_saved_tracks.push(page_saved_tracks);
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
    pub fn request_current_user_saved_tracks(&self) {
        self.tx
            .send(SpotifyAPIEvent::CurrentUserSavedTracks(None))
            .unwrap();
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
    pub fn shuffle(&self) {
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
            if current_playback.shuffle_state {
                self.tx
                    .send(SpotifyAPIEvent::Shuffle(false, Some(device_id)))
                    .unwrap();
            } else {
                self.tx
                    .send(SpotifyAPIEvent::Shuffle(true, Some(device_id)))
                    .unwrap();
            }
        }
    }
    pub fn request_next_track(&self) {
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
        self.tx
            .send(SpotifyAPIEvent::NextTrack(Some(device_id)))
            .unwrap();
    }
    pub fn request_previous_track(&self) {
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
        self.tx
            .send(SpotifyAPIEvent::PreviousTrack(Some(device_id)))
            .unwrap();
    }
    pub fn request_volume(&self, up: bool) {
        if let Some(current_playback) = self.spotify_data.current_playback.as_ref() {
            let mut next_volume = current_playback.device.volume_percent as u8;
            if up {
                next_volume = cmp::min(next_volume + 6, 100);
            } else {
                if next_volume >= 6 {
                    next_volume -= 6;
                } else {
                    next_volume = 0;
                }
            }
            self.tx.send(SpotifyAPIEvent::Volume(next_volume, Some(current_playback.device.id.clone()))).unwrap();
        }
    }

    pub fn request_repeat(&self) {
        if let Some(current_playback) = self.spotify_data.current_playback.as_ref() {
            match current_playback.repeat_state {
                RepeatState::Off => {
                    self.tx.send(SpotifyAPIEvent::Repeat(RepeatState::Track, Some(current_playback.device.id.clone()))).unwrap();
                }
                RepeatState::Track => {
                    self.tx.send(SpotifyAPIEvent::Repeat(RepeatState::Context, Some(current_playback.device.id.clone()))).unwrap();
                }
                RepeatState::Context => {
                    self.tx.send(SpotifyAPIEvent::Repeat(RepeatState::Off, Some(current_playback.device.id.clone()))).unwrap();
                }
            }
        }
    }

    pub fn player_items(&self) -> Vec<Text> {
        let mut items = vec![];
        if let Some(current_playback) = self.spotify_data.current_playback.as_ref() {
            if let Some(playing_track) = current_playback.item.as_ref() {
                items.push(Text::styled(
                    format!(
                        "🎵 Song: {} |🎤 Artist: {} | 💿 Album: {}",
                        playing_track.name, playing_track.artists[0].name, playing_track.album.name
                    ),
                    Style::default().fg(Color::White),
                ));
                //Status
                let playing_icon = if current_playback.is_playing {
                    //headphone
                    "🎧"
                } else {
                    //stop
                    "⏹️"
                };
                let shuffle_state_icon = if current_playback.shuffle_state {
                    //shuffle
                    "🔀"
                } else {
                    "❌"
                };
                let repeat_state_icon = match current_playback.repeat_state {
                    RepeatState::Context => "🔁 🎵",
                    RepeatState::Track => "🔂 💿",
                    _ => "❌",
                };
                let duration_sec = playing_track.duration_ms / 1000;
                let duration = format!("{:02}:{:02}", duration_sec / 60, duration_sec % 60);
                let progress_sec = current_playback.progress_ms.unwrap_or(0) / 1000;
                let progress = format!("{:02}:{:02}", progress_sec / 60, progress_sec % 60);

                items.push(Text::styled(
                    format!(
                        "Progress: {} / {} | Playing: {}  | Shuffle: {} | Repeat:  {}",
                        progress, duration, playing_icon, shuffle_state_icon, repeat_state_icon
                    ),
                    Style::default(),
                ));
            }

            items.push(Text::styled(
                format!(
                    "🔊 Volume: {} | 💻 Device Name: {}",
                    current_playback.device.volume_percent, current_playback.device.name
                ),
                Style::default(),
            ));
        }
        return items;
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
            .clone()
            .id;
        if let Some(current_playback) = self.spotify_data.current_playback.as_ref() {
            //pause
            if current_playback.is_playing {
                self.tx
                    .send(SpotifyAPIEvent::Pause(Some(device_id.clone())))
                    .unwrap();
            } else {
                //unpause
                self.tx
                    .send(SpotifyAPIEvent::StartPlayBack((
                        Some(device_id.clone()),
                        None,
                    )))
                    .unwrap();
            }
        } else {
            self.tx
                .send(SpotifyAPIEvent::Pause(Some(device_id.clone())))
                .unwrap();
        }
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
