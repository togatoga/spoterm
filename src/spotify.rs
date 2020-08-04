extern crate crossbeam;
extern crate rspotify;

use self::rspotify::client;
use self::rspotify::model;
use self::rspotify::model::page::Page;
use self::rspotify::model::track::SavedTrack;
use self::rspotify::oauth2;
use self::rspotify::senum::RepeatState;
use std::thread;
use tokio::prelude::*;

pub enum SpotifyAPIEvent {
    Shuffle(bool, Option<String>),
    Pause(Option<String>),
    Device,
    Volume(u8, Option<String>),
    Repeat(RepeatState, Option<String>),
    SeekTrack(u32, Option<String>),
    NextTrack(Option<String>),
    PreviousTrack(Option<String>),
    CurrentPlayBack,
    CurrentUserRecentlyPlayed,
    DeleteCurrentUserSavedTracks(Vec<String>),
    AddCurrentUserSavedTracks(Vec<String>),
    CheckCurrentUserSavedTracks(Vec<String>),
    CurrentUserSavedTracks(Option<u32>), //offset
    StartPlayBack((Option<String>, Option<Vec<String>>)),
}

pub enum SpotifyAPIResult {
    CurrentPlayBack(Option<model::context::CurrentlyPlaybackContext>),
    CurrentUserPlayingTrack(Option<model::playing::Playing>),
    CurrentUserRecentlyPlayed(Vec<model::playing::PlayHistory>),
    CheckCurrentUserSavedTracks(Vec<(String, bool)>),
    CurrentUserSavedTracks(Page<SavedTrack>),
    Device(Vec<model::device::Device>),
    SuccessAddCurrentUserSavedTracks(Vec<String>),
    SuccessDeleteCurrentUserSavedTracks(Vec<String>),
}

pub struct SpotifyService {
    pub client: client::Spotify,
    pub oauth: rspotify::oauth2::SpotifyOAuth,
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
    pub fn new(
        token_info: rspotify::oauth2::TokenInfo,
        oauth: rspotify::oauth2::SpotifyOAuth,
    ) -> SpotifyService {
        let client_credential = rspotify::oauth2::SpotifyClientCredentials::default()
            .token_info(token_info)
            .build();
        let spotify = rspotify::client::Spotify::default()
            .client_credentials_manager(client_credential)
            .build();

        let (tx, rx) = crossbeam::channel::unbounded();

        SpotifyService {
            client: spotify,
            oauth: oauth,
            api_result_tx: None,
            api_event_tx: tx,
            api_event_rx: rx,
        }
    }
    pub fn api_result_tx(mut self, tx: crossbeam::channel::Sender<SpotifyAPIResult>) -> Self {
        self.api_result_tx = Some(tx);
        self
    }

    pub async fn refresh_client(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let now = std::time::SystemTime::now();
        let unixtime = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
        let old_token = self
            .client
            .client_credentials_manager
            .clone()
            .unwrap()
            .token_info
            .unwrap();
        let expires_at_unix_time = old_token.expires_at.unwrap();
        let refresh = if expires_at_unix_time <= unixtime {
            true
        } else {
            false
        };
        if refresh {
            let refresh_token = old_token.refresh_token.unwrap();
            let token_info = self
                .oauth
                .refresh_access_token(&refresh_token)
                .await
                .unwrap();
            log::info!("{:?}", token_info);

            let client_credential = rspotify::oauth2::SpotifyClientCredentials::default()
                .token_info(token_info)
                .build();
            let spotify = rspotify::client::Spotify::default()
                .client_credentials_manager(client_credential)
                .build();

            self.client = spotify;
        }
        Ok(())
    }
    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error>> {
        let rx = self.api_event_rx.clone();

        tokio::spawn(async move {
            loop {
                self.refresh_client().await;
                match rx.recv().unwrap() {
                    SpotifyAPIEvent::Shuffle(state, device_id) => {
                        self.fetch_shuffle(state, device_id).await;
                    }
                    SpotifyAPIEvent::Pause(device_id) => {
                        self.fetch_pause_playback(device_id).await;
                    }
                    SpotifyAPIEvent::Device => {
                        self.fetch_device().await;
                    }
                    SpotifyAPIEvent::SeekTrack(progress_ms, device_id) => {
                        self.fetch_seek_track(progress_ms, device_id).await;
                    }
                    SpotifyAPIEvent::Volume(volume_percent, device_id) => {
                        self.fetch_volume(volume_percent, device_id).await;
                    }
                    SpotifyAPIEvent::Repeat(state, device_id) => {
                        self.fetch_repeat(state, device_id).await;
                    }
                    SpotifyAPIEvent::NextTrack(device_id) => {
                        self.fetch_next_track(device_id).await;
                    }
                    SpotifyAPIEvent::PreviousTrack(device_id) => {
                        self.fetch_previous_track(device_id).await;
                    }
                    SpotifyAPIEvent::CurrentUserRecentlyPlayed => {
                        self.fetch_current_user_recently_played().await;
                    }
                    SpotifyAPIEvent::DeleteCurrentUserSavedTracks(track_ids) => {
                        self.fetch_delete_current_user_saved_tracks(&track_ids)
                            .await;
                    }
                    SpotifyAPIEvent::AddCurrentUserSavedTracks(track_ids) => {
                        self.fetch_add_current_user_saved_tracks(&track_ids).await;
                    }
                    SpotifyAPIEvent::CheckCurrentUserSavedTracks(track_ids) => {
                        self.fetch_check_current_user_saved_tracks(&track_ids).await;
                    }
                    SpotifyAPIEvent::CurrentUserSavedTracks(offset) => {
                        self.fetch_current_user_saved_tracks(offset).await;
                    }
                    SpotifyAPIEvent::StartPlayBack((device_id, uris)) => {
                        self.fetch_start_playback(device_id, uris).await;
                    }
                    SpotifyAPIEvent::CurrentPlayBack => {
                        self.fetch_current_playback().await;
                    }
                    _ => {}
                }
            }
        });
        Ok(())
    }
    async fn fetch_check_current_user_saved_tracks(
        &self,
        track_ids: &Vec<String>,
    ) -> Result<(), failure::Error> {
        let saved_tracks = self
            .client
            .current_user_saved_tracks_contains(track_ids)
            .await?;
        let result: Vec<(String, bool)> = track_ids
            .iter()
            .zip(saved_tracks.iter())
            .map(|(x, y)| (x.clone(), *y))
            .collect();
        self.api_result_tx
            .clone()
            .unwrap()
            .send(SpotifyAPIResult::CheckCurrentUserSavedTracks(result))?;
        Ok(())
    }
    async fn fetch_start_playback(
        &self,
        device_id: Option<String>,
        uris: Option<Vec<String>>,
    ) -> Result<(), failure::Error> {
        self.client
            .start_playback(device_id, None, uris, None, None)
            .await
    }
    async fn fetch_current_user_recently_played(&self) -> Result<(), failure::Error> {
        let items = self.client.current_user_recently_played(50).await?.items;
        self.api_result_tx
            .clone()
            .unwrap()
            .send(SpotifyAPIResult::CurrentUserRecentlyPlayed(items))?;
        Ok(())
    }
    async fn fetch_current_playback(&self) -> Result<(), failure::Error> {
        let current_playback = self.client.current_playback(None, None).await?;
        self.api_result_tx
            .clone()
            .unwrap()
            .send(SpotifyAPIResult::CurrentPlayBack(current_playback))?;
        Ok(())
    }
    async fn fetch_current_user_playing_track(&self) -> Result<(), failure::Error> {
        let playing_track = self.client.current_user_playing_track().await?;
        self.api_result_tx
            .clone()
            .unwrap()
            .send(SpotifyAPIResult::CurrentUserPlayingTrack(playing_track))?;
        Ok(())
    }
    async fn fetch_seek_track(
        &self,
        progress_ms: u32,
        device_id: Option<String>,
    ) -> Result<(), failure::Error> {
        self.client.seek_track(progress_ms, device_id).await?;
        Ok(())
    }
    async fn fetch_repeat(
        &self,
        state: RepeatState,
        device_id: Option<String>,
    ) -> Result<(), failure::Error> {
        self.client.repeat(state, device_id).await?;
        Ok(())
    }
    async fn fetch_volume(
        &self,
        volume_percent: u8,
        device_id: Option<String>,
    ) -> Result<(), failure::Error> {
        self.client.volume(volume_percent, device_id).await?;
        Ok(())
    }
    async fn fetch_device(&self) -> Result<(), failure::Error> {
        let devices = self.client.device().await?.devices;
        self.api_result_tx
            .clone()
            .unwrap()
            .send(SpotifyAPIResult::Device(devices))?;
        Ok(())
    }
    async fn fetch_delete_current_user_saved_tracks(
        &self,
        track_ids: &Vec<String>,
    ) -> Result<(), failure::Error> {
        self.client
            .current_user_saved_tracks_delete(&track_ids)
            .await?;
        self.api_result_tx.clone().unwrap().send(
            SpotifyAPIResult::SuccessDeleteCurrentUserSavedTracks(track_ids.clone()),
        )?;
        Ok(())
    }
    async fn fetch_add_current_user_saved_tracks(
        &self,
        track_ids: &Vec<String>,
    ) -> Result<(), failure::Error> {
        self.client.current_user_saved_tracks_add(track_ids).await?;
        self.api_result_tx.clone().unwrap().send(
            SpotifyAPIResult::SuccessAddCurrentUserSavedTracks(track_ids.clone()),
        )?;
        Ok(())
    }
    async fn fetch_current_user_saved_tracks(
        &self,
        offset: Option<u32>,
    ) -> Result<(), failure::Error> {
        let saved_tracks = self
            .client
            .current_user_saved_tracks(Some(50), offset)
            .await?;
        self.api_result_tx
            .clone()
            .unwrap()
            .send(SpotifyAPIResult::CurrentUserSavedTracks(saved_tracks))?;
        Ok(())
    }
    async fn fetch_shuffle(
        &self,
        state: bool,
        device_id: Option<String>,
    ) -> Result<(), failure::Error> {
        self.client.shuffle(state, device_id).await?;
        Ok(())
    }
    async fn fetch_pause_playback(&self, device_id: Option<String>) -> Result<(), failure::Error> {
        self.client.pause_playback(device_id).await?;
        Ok(())
    }
    async fn fetch_previous_track(&self, device_id: Option<String>) -> Result<(), failure::Error> {
        self.client.previous_track(device_id).await?;
        Ok(())
    }
    async fn fetch_next_track(&self, device_id: Option<String>) -> Result<(), failure::Error> {
        self.client.next_track(device_id).await?;
        Ok(())
    }
}
