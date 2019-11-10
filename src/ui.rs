use crate::spoterm::SpotifyData;
use crate::spotify::SpotifyAPIEvent;
use itertools::Itertools;
use rspotify::spotify::model::playing::PlayHistory;
use rspotify::spotify::model::track::SavedTrack;
use termion;
use termion::input::MouseTerminal;
use termion::raw::RawTerminal;
use tui;
use tui::widgets::{Block, Borders, SelectableList, Widget};
use unicode_width;

pub trait UI {
    fn key_down(&mut self);
    fn key_up(&mut self);
    fn key_enter(&mut self);
    fn set_data(&mut self, data: &SpotifyData);
    fn set_filter(&mut self, filter: String);
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
    pub filter: String,
    pub input_mode: bool,
}

impl Contents {
    pub fn new() -> Contents {
        Contents {
            uis: vec![],
            filter: String::default(),
            input_mode: false,
        }
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
    pub filter: String,
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
    fn set_filter(&mut self, filter: String) {
        self.filter = filter;
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
            filter: String::default(),
            tx,
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
    pub filter: String,
    pub tx: crossbeam::channel::Sender<SpotifyAPIEvent>,
}

impl LikedSongs {
    pub fn new(tx: crossbeam::channel::Sender<SpotifyAPIEvent>) -> LikedSongs {
        LikedSongs {
            selected_id: None,
            device_id: None,
            saved_tracks: Vec::new(),
            filter: String::default(),
            tx,
        }
    }
    fn trim_text(text: &String, width_max_limit: usize) -> String {
        let mut result = String::new();
        text.chars().for_each(|x| {
            let size = unicode_width::UnicodeWidthChar::width(x).unwrap_or_default();
            let current_size = unicode_width::UnicodeWidthStr::width(result.as_str());
            if current_size + size <= width_max_limit {
                result.push(x.clone());
            }
        });
        while unicode_width::UnicodeWidthStr::width(result.as_str()) < width_max_limit {
            result.push(' ');
        }
        result
    }
    fn filter_saved_tracks(&self) -> Vec<&SavedTrack> {
        let filter = self.filter.to_ascii_lowercase();
        let saved_tracks: Vec<&SavedTrack> = self
            .saved_tracks
            .iter()
            .filter(|&save_track| {
                if self.filter == "" {
                    true
                } else {
                    let lower_track_name = save_track.track.name.to_ascii_lowercase();
                    if lower_track_name.contains(&filter) {
                        return true;
                    }
                    let lower_album_name = save_track.track.album.name.to_ascii_lowercase();
                    if lower_album_name.contains(&filter) {
                        return true;
                    }
                    let lower_artist = save_track
                        .track
                        .artists
                        .iter()
                        .map(|x| x.name.clone().to_ascii_lowercase())
                        .join(" ");
                    if lower_artist.contains(&filter) {
                        return true;
                    }
                    false
                }
            })
            .collect();
        saved_tracks
    }
    fn items_from_saved_tracks(&self) -> Vec<String> {
        let mut items = vec![];

        for saved_track in self.filter_saved_tracks().iter() {
            let track = LikedSongs::trim_text(&saved_track.track.name, 30);
            let artist = LikedSongs::trim_text(
                &saved_track
                    .track
                    .artists
                    .iter()
                    .map(|x| x.name.clone())
                    .join(" "),
                20,
            );
            let album = LikedSongs::trim_text(&saved_track.track.album.name, 20);
            let popularity = LikedSongs::trim_text(&format!("{}", saved_track.track.popularity), 3);
            let total_sec = saved_track.track.duration_ms / 1000;
            let duration = format!("{:02}:{:02}", total_sec / 60, total_sec % 60);
            let added_at = saved_track.added_at.format("%Y-%m-%d %H:%M:%S").to_string();

            let line = format!(
                "❤   {}     {}     {}     {}     {}     {}",
                track, artist, album, duration, popularity, added_at
            );
            items.push(line);
        }
        items
    }
}

impl UI for LikedSongs {
    fn key_down(&mut self) {
        let max_track_size = self.filter_saved_tracks().len();
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
                self.selected_id = Some(self.filter_saved_tracks().len() - 1);
            }
        } else {
            self.selected_id = Some(0);
        }
    }
    fn key_enter(&mut self) {
        if self.selected_id.is_none() || self.filter_saved_tracks().is_empty() {
            return;
        }
        let selected_id = self.selected_id.unwrap();
        let saved_tracks = self.filter_saved_tracks().clone();

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
        self.saved_tracks = data.saved_tracks.clone();

        if let Some(device) = data.selected_device.as_ref() {
            self.device_id = Some(device.clone().id);
        }
    }
    fn set_filter(&mut self, filter: String) {
        self.filter = filter;
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
