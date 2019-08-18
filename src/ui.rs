use crate::spoterm::SpotifyData;
use crate::spotify::SpotifyAPIEvent;
use core::borrow::BorrowMut;
use itertools::Itertools;
use rspotify::spotify::client::Spotify;
use rspotify::spotify::model::playing::{PlayHistory, Playing};
use rspotify::spotify::model::track::SavedTrack;
use termion;
use termion::input::{MouseTerminal, TermRead};
use termion::raw::RawTerminal;
use tui;
use tui::backend::TermionBackend;
use tui::widgets::{Block, Borders, SelectableList, Tabs, Widget};

pub trait UI {
    fn key_down(&mut self);
    fn key_up(&mut self);
    fn key_enter(&mut self);
    fn set_data(&mut self, data: &SpotifyData);
    fn render(
        &self,
        f: &mut tui::terminal::Frame<
            tui::backend::TermionBackend<
                termion::screen::AlternateScreen<MouseTerminal<RawTerminal<std::io::Stdout>>>,
            >,
        >,
        area: tui::layout::Rect,
    );
}

pub struct Contents {
    pub uis: Vec<Box<UI>>,
}

impl Contents {
    pub fn new() -> Contents {
        Contents { uis: vec![] }
    }
    pub fn ui<T: 'static + UI>(mut self, ui: T) -> Self {
        self.uis.push(Box::new(ui));
        self
    }
}

#[derive(Clone, Debug)]
pub struct RecentPlayed {
    pub selected_id: Option<usize>,
    pub device_id: Option<String>,
    pub recent_play_histories: Option<Vec<PlayHistory>>,
    pub tx: crossbeam::channel::Sender<SpotifyAPIEvent>,
}

impl UI for RecentPlayed {
    fn key_down(&mut self) {
        let max_track_size = self.recent_play_histories.clone().unwrap().len();
        if let Some(selected) = self.selected_id {
            if selected + 1 < max_track_size {
                self.selected_id = Some(selected + 1);
            } else {
                self.selected_id = Some(0);
            }
        } else {
            self.selected_id = Some(0);
        }
    }
    fn key_up(&mut self) {
        if let Some(selected) = self.selected_id {
            if selected > 0 {
                self.selected_id = Some(selected - 1);
            } else {
                self.selected_id = Some(self.recent_play_histories.clone().unwrap().len() - 1);
            }
        } else {
            self.selected_id = Some(0);
        }
    }
    fn key_enter(&mut self) {
        if self.selected_id.is_none() || self.recent_play_histories.is_none() {
            return;
        }
        let selected_id = self.selected_id.unwrap();
        let play_histories = self.recent_play_histories.clone().unwrap();
        let mut uris = vec![];
        for play_history in play_histories.iter().skip(selected_id) {
            uris.push(format!(
                "spotify:track:{}",
                play_history.track.id.clone().unwrap()
            ));
        }
        self.tx
            .send(SpotifyAPIEvent::StartPlayBack((
                self.device_id.clone(),
                Some(uris),
            )))
            .unwrap();
    }
    fn set_data(&mut self, data: &SpotifyData) {
        self.recent_play_histories = data.recent_play_histories.clone();
        if let Some(device) = data.selected_device.as_ref() {
            self.device_id = Some(device.clone().id);
        }
    }
    fn render(
        &self,
        f: &mut tui::terminal::Frame<
            tui::backend::TermionBackend<
                termion::screen::AlternateScreen<MouseTerminal<RawTerminal<std::io::Stdout>>>,
            >,
        >,
        area: tui::layout::Rect,
    ) {
        SelectableList::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Recently Played"),
            )
            .items(&self.items_from_play_history(self.recent_play_histories.clone()))
            .select(self.selected_id)
            .highlight_symbol(">")
            .render(f, area);
    }
}

impl RecentPlayed {
    pub fn new(tx: crossbeam::channel::Sender<SpotifyAPIEvent>) -> RecentPlayed {
        RecentPlayed {
            selected_id: None,
            device_id: None,
            recent_play_histories: None,
            tx: tx,
        }
    }
    fn items_from_play_history(&self, play_histories: Option<Vec<PlayHistory>>) -> Vec<String> {
        if play_histories.is_none() {
            return vec![];
        }
        let play_histories = play_histories.as_ref().unwrap();
        //let play_history_items: Vec<PlayHistory> = .items.into_iter().unique_by(|x| x.track.clone().id).collect();

        let mut items = vec![];
        let max_track_name_width = play_histories
            .iter()
            .map(|x| unicode_width::UnicodeWidthStr::width(x.track.name.as_str()))
            .max()
            .unwrap_or(0)
            + 15;
        for history in play_histories.iter() {
            let mut whitespace: String = "".to_string();
            let mut tmp = history.track.name.clone() + &whitespace;
            while unicode_width::UnicodeWidthStr::width(tmp.as_str()) < max_track_name_width {
                whitespace += " ";
                tmp = history.track.name.clone() + &whitespace;
            }
            items.push(format!(
                "{}{}{}",
                history.track.name, whitespace, history.track.artists[0].name
            ));
        }
        items
    }
}

#[derive(Clone, Debug)]
pub struct LikedSongs {
    pub selected_id: Option<usize>,
    pub device_id: Option<String>,
    pub saved_tracks: Vec<SavedTrack>,
    pub tx: crossbeam::channel::Sender<SpotifyAPIEvent>,
}

impl LikedSongs {
    pub fn new(tx: crossbeam::channel::Sender<SpotifyAPIEvent>) -> LikedSongs {
        LikedSongs {
            selected_id: None,
            device_id: None,
            saved_tracks: Vec::new(),
            tx: tx,
        }
    }
    fn items_from_saved_tracks(&self) -> Vec<String> {
        let max_track_name_width = self
            .saved_tracks
            .iter()
            .map(|x| unicode_width::UnicodeWidthStr::width(x.track.name.as_str()))
            .max()
            .unwrap_or(0)
            + 15;

        let mut items = vec![];

        for saved_track in self.saved_tracks.iter() {
            let mut whitespace: String = "".to_string();
            let mut tmp = saved_track.track.name.clone() + &whitespace;
            while unicode_width::UnicodeWidthStr::width(tmp.as_str()) < max_track_name_width {
                whitespace += " ";
                tmp = saved_track.track.name.clone() + &whitespace;
            }
            items.push(format!(
                "❤  {}{}{}",
                saved_track.track.name, whitespace, saved_track.track.artists[0].name
            ));
        }
        items
    }
}

impl UI for LikedSongs {
    fn key_down(&mut self) {
        let max_track_size = self.saved_tracks.len();
        if let Some(selected) = self.selected_id {
            if selected + 1 < max_track_size {
                self.selected_id = Some(selected + 1);
            } else {
                self.selected_id = Some(0);
            }
        } else {
            self.selected_id = Some(0);
        }
    }
    fn key_up(&mut self) {
        if let Some(selected) = self.selected_id {
            log::info!("{}", selected);
            if selected > 0 {
                self.selected_id = Some(selected - 1);
            } else {
                self.selected_id = Some(self.saved_tracks.len() - 1);
            }
        } else {
            self.selected_id = Some(0);
        }
    }
    fn key_enter(&mut self) {
        if self.selected_id.is_none() || self.saved_tracks.is_empty() {
            return;
        }
        let selected_id = self.selected_id.unwrap();
        let saved_tracks = self.saved_tracks.clone();

        let mut uris = vec![];

        //let saved_track = saved_tracks.windows(selected_id);
        for saved_track in saved_tracks.iter().skip(selected_id) {
            if let Some(id) = saved_track.track.id.as_ref() {
                uris.push(format!("spotify:track:{}", id));
            }
        }
        self.tx
            .send(SpotifyAPIEvent::StartPlayBack((
                self.device_id.clone(),
                Some(uris),
            )))
            .unwrap();
    }
    fn set_data(&mut self, data: &SpotifyData) {
        self.saved_tracks.clear();
        for page_saved_track in data.page_saved_tracks.clone() {
            for saved_track in page_saved_track.items {
                self.saved_tracks.push(saved_track);
            }
        }

        if let Some(device) = data.selected_device.as_ref() {
            self.device_id = Some(device.clone().id);
        }
    }
    fn render(
        &self,
        f: &mut tui::terminal::Frame<
            tui::backend::TermionBackend<
                termion::screen::AlternateScreen<MouseTerminal<RawTerminal<std::io::Stdout>>>,
            >,
        >,
        area: tui::layout::Rect,
    ) {
        SelectableList::default()
            .block(Block::default().borders(Borders::ALL).title("Liked Songs"))
            .items(&self.items_from_saved_tracks())
            .select(self.selected_id)
            .highlight_symbol(">")
            .render(f, area);
    }
}
