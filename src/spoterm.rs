extern crate failure;
extern crate hostname;
extern crate itertools;
extern crate rspotify;
extern crate unicode_width;
use itertools::Itertools;

use self::rspotify::spotify::model::playing::Playing;
use crate::spotify::{SpotifyAPIEvent, SpotifyAPIResult, SpotifyService};
use crate::ui::{Contents, LikedSongs, RecentPlayed};

use self::rspotify::spotify::model::context::FullPlayingContext;
use self::rspotify::spotify::model::page::Page;
use self::rspotify::spotify::model::track::SavedTrack;
use crate::spoterm::SaveState::{CHECKING, SAVED, UNKNOWN};
use rspotify::spotify::client::Spotify;
use rspotify::spotify::model::device::Device;
use rspotify::spotify::model::playing::PlayHistory;
use rspotify::spotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth};
use rspotify::spotify::senum::RepeatState;
use rspotify::spotify::util::get_token;
use std::cmp;
use std::collections::HashMap;
use std::thread;
use tui::style::{Color, Style};
use tui::widgets::Text;

#[derive(Clone, Debug)]
pub enum SaveState {
    SAVED,
    UNSAVED,
    SAVING,
    UNSAVING,
    CHECKING,
    UNKNOWN,
}

#[derive(Clone, Debug)]
pub struct SpotifyData {
    pub devices: Option<Vec<Device>>,
    pub saved_tracks: Vec<SavedTrack>,
    pub recent_play_histories: Option<Vec<PlayHistory>>,
    pub current_playback: Option<FullPlayingContext>,
    pub selected_device: Option<Device>,
    pub save_state_track_ids: HashMap<String, SaveState>,
}

impl SpotifyData {
    pub fn new() -> SpotifyData {
        SpotifyData {
            devices: None,
            saved_tracks: Vec::new(),
            recent_play_histories: None,
            current_playback: None,
            selected_device: None,
            save_state_track_ids: HashMap::new(),
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
                SpotifyAPIResult::CheckCurrentUserSavedTracks(saved_tracks) => {
                    for (track_id, saved) in saved_tracks.iter() {
                        if *saved {
                            self.spotify_data
                                .save_state_track_ids
                                .insert(track_id.clone(), SaveState::SAVED);
                        } else {
                            self.spotify_data
                                .save_state_track_ids
                                .insert(track_id.clone(), SaveState::UNSAVED);
                        }
                    }
                }
                SpotifyAPIResult::SuccessAddCurrentUserSavedTracks(track_ids) => {
                    for track_id in track_ids {
                        self.spotify_data
                            .save_state_track_ids
                            .insert(track_id, SaveState::SAVED);
                    }
                    self.request_current_user_saved_tracks();
                }
                SpotifyAPIResult::SuccessDeleteCurrentUserSavedTracks(track_ids) => {
                    self.spotify_data.saved_tracks.retain(|x| {
                        let track_id = x.track.id.as_ref().unwrap();
                        if track_ids.contains(track_id) {
                            return false;
                        }
                        true
                    });
                }
                SpotifyAPIResult::CurrentUserSavedTracks(page_saved_tracks) => {
                    let pre_track_ids: Vec<String> = self
                        .spotify_data
                        .saved_tracks
                        .iter()
                        .map(|x| x.track.id.clone().unwrap())
                        .collect();
                    self.spotify_data
                        .saved_tracks
                        .append(&mut page_saved_tracks.items.clone());
                    //unique
                    self.spotify_data
                        .saved_tracks
                        .sort_by_key(|x| x.track.id.clone());
                    self.spotify_data
                        .saved_tracks
                        .dedup_by_key(|x| x.track.id.clone().unwrap_or("".to_string()));

                    self.spotify_data.saved_tracks.sort_by_key(|x| x.added_at);
                    self.spotify_data.saved_tracks.reverse();

                    let new_track_ids: Vec<String> = self
                        .spotify_data
                        .saved_tracks
                        .iter()
                        .map(|x| x.track.id.clone().unwrap())
                        .collect();
                    if pre_track_ids != new_track_ids && page_saved_tracks.next.is_some() {
                        log::info!("Request!!");
                        self.tx
                            .send(SpotifyAPIEvent::CurrentUserSavedTracks(Some(
                                page_saved_tracks.offset + page_saved_tracks.limit,
                            )))
                            .unwrap();
                    }
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
    pub fn request_save_current_playback(&mut self) {
        if let Some(current_playback) = self.spotify_data.current_playback.as_ref() {
            if let Some(current_playback) = current_playback.item.as_ref() {
                if let Some(track_id) = current_playback.id.as_ref() {
                    //doesn't exist
                    if let Some(save_state) = self.spotify_data.save_state_track_ids.get(track_id) {
                        match save_state {
                            SaveState::SAVED | SaveState::SAVING => {
                                self.tx
                                    .send(SpotifyAPIEvent::DeleteCurrentUserSavedTracks(vec![
                                        track_id.clone(),
                                    ]))
                                    .unwrap();
                                self.spotify_data
                                    .save_state_track_ids
                                    .insert(track_id.clone(), SaveState::UNSAVING);
                            }
                            SaveState::UNSAVED | SaveState::UNSAVING => {
                                self.tx
                                    .send(SpotifyAPIEvent::AddCurrentUserSavedTracks(vec![
                                        track_id.clone(),
                                    ]))
                                    .unwrap();
                                self.spotify_data
                                    .save_state_track_ids
                                    .insert(track_id.clone(), SaveState::SAVING);
                            }
                            _ => {}
                        }
                    }
                }
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
    pub fn request_seek_to_zero_or_previous_track(&self) {
        if let Some(device) = self.spotify_data.selected_device.as_ref() {
            if let Some(current_playback) = self.spotify_data.current_playback.as_ref() {
                let progress_ms = current_playback.progress_ms.unwrap_or(0);
                if progress_ms <= 3000 {
                    self.request_previous_track();
                } else {
                    self.tx
                        .send(SpotifyAPIEvent::SeekTrack(0, Some(device.id.clone())))
                        .unwrap();
                }
            }
        }
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
            self.tx
                .send(SpotifyAPIEvent::Volume(
                    next_volume,
                    Some(current_playback.device.id.clone()),
                ))
                .unwrap();
        }
    }
    pub fn request_check_unknown_saved_tracks(&mut self) {
        let mut unknown_track_ids = Vec::new();
        for (id, state) in self.spotify_data.save_state_track_ids.iter_mut() {
            match state {
                SaveState::UNKNOWN => {
                    unknown_track_ids.push(id.clone());
                    *state = SaveState::CHECKING;
                }
                _ => {}
            }
        }
        self.tx
            .send(SpotifyAPIEvent::CheckCurrentUserSavedTracks(
                unknown_track_ids,
            ))
            .unwrap();
    }
    pub fn request_repeat(&self) {
        if let Some(current_playback) = self.spotify_data.current_playback.as_ref() {
            match current_playback.repeat_state {
                RepeatState::Off => {
                    self.tx
                        .send(SpotifyAPIEvent::Repeat(
                            RepeatState::Track,
                            Some(current_playback.device.id.clone()),
                        ))
                        .unwrap();
                }
                RepeatState::Track => {
                    self.tx
                        .send(SpotifyAPIEvent::Repeat(
                            RepeatState::Context,
                            Some(current_playback.device.id.clone()),
                        ))
                        .unwrap();
                }
                RepeatState::Context => {
                    self.tx
                        .send(SpotifyAPIEvent::Repeat(
                            RepeatState::Off,
                            Some(current_playback.device.id.clone()),
                        ))
                        .unwrap();
                }
            }
        }
    }

    pub fn player_items(&mut self) -> Vec<Text> {
        let mut items = vec![];
        if let Some(current_playback) = self.spotify_data.current_playback.clone() {
            if let Some(playing_track) = current_playback.item.clone() {
                let track_id = playing_track.id.clone().unwrap_or("".to_string());
                let like_track_icon = match self.save_state_track(track_id.clone()) {
                    SaveState::SAVED | SaveState::SAVING => "❤",
                    SaveState::UNSAVED | SaveState::UNSAVING => "♡",
                    _ => "❓",
                };
                items.push(Text::styled(
                    format!(
                        "🎵  {} Song: {} |🎤 Artist: {} | 💿 Album: {}",
                        like_track_icon,
                        playing_track.name,
                        playing_track.artists[0].name,
                        playing_track.album.name
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
                    RepeatState::Context => "🔁 💿",
                    RepeatState::Track => "🔂 🎵",
                    _ => "❌",
                };
                let duration_sec = playing_track.duration_ms / 1000;
                let duration = format!("{:02}:{:02}", duration_sec / 60, duration_sec % 60);
                let progress_sec = current_playback.progress_ms.unwrap_or(0) / 1000;
                let progress = format!("{:02}:{:02}", progress_sec / 60, progress_sec % 60);

                items.push(Text::styled(
                    format!(
                        "    Progress: {} / {} | Playing: {}  | Shuffle: {} | Repeat:  {}",
                        progress, duration, playing_icon, shuffle_state_icon, repeat_state_icon
                    ),
                    Style::default(),
                ));
            }

            items.push(Text::styled(
                format!(
                    "🔊  Volume: {} | 💻 Device: {}",
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
    fn save_state_track(&mut self, id: String) -> SaveState {
        if let Some(state) = self.spotify_data.save_state_track_ids.get(&id) {
            return state.clone();
        }
        self.spotify_data
            .save_state_track_ids
            .insert(id.clone(), UNKNOWN);
        return SaveState::UNKNOWN;
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
