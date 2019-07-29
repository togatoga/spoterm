use core::borrow::BorrowMut;
use rspotify::spotify::model::playing::{PlayHistory, Playing};
use tui::widgets::{Block, Borders, SelectableList, Tabs, Widget};
use tui;


pub trait UI {}

pub struct ContentsUI {
    pub current_ui_index: Option<usize>,
    pub uis: Vec<Box<UI>>,
}

impl ContentsUI {
    pub fn new() -> ContentsUI {
        ContentsUI{ current_ui_index: None, uis: vec![]}
    }
    pub fn ui<T: 'static + UI>(mut self, ui: T) -> Self {
        self.uis.push(Box::new(ui));
        self.current_ui_index = Some(0);
        self
    }

}

#[derive(Clone, Debug)]
pub struct RecentPlayed {
    pub selected_id: Option<usize>,
    pub recent_play_histories: Option<Vec<PlayHistory>>,
    pub items: Vec<String>,
}

impl UI for RecentPlayed {
}
impl RecentPlayed{
    pub fn new() -> RecentPlayed {
        RecentPlayed {
            selected_id: None,
            recent_play_histories: None,
            items: vec![],
        }
    }

    pub fn render<B>(&self, f: &mut tui::terminal::Frame<B>, area: tui::layout::Rect) where B:tui::backend::Backend {
        SelectableList::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Recently Played"),
            )
            .items(&self.items)
            .select(self.selected_id)
            .highlight_symbol(">").render(f, area);
    }
    pub fn update_selected_id(&mut self, playing: Option<Playing>) {

        if playing.is_some() {
            let playing = playing.unwrap();
            if playing.item.is_some() {
                let playing_track_id = playing.item.unwrap().id.unwrap_or("0".to_string());
                match self.recent_play_histories.as_ref() {
                    Some(histories) => {
                        for (selected_id, history) in histories.clone().into_iter().enumerate() {
                            if let Some(track_id) = history.track.id.as_ref() {
                                if *track_id == playing_track_id {
                                    self.selected_id = Some(selected_id);
                                    break;
                                }
                            }
                        }
                    },
                    _ => {}
                }
            }
        }

    }
    pub fn key_enter(&self) -> Vec<String> {
        if self.selected_id.is_none() || self.recent_play_histories.is_none() {
            return vec![];
        }
        let selected_id = self.selected_id.unwrap();
        let play_histories = self.recent_play_histories.clone().unwrap();
        let mut uris = vec![];
        for idx in selected_id..play_histories.len() {
            uris.push(format!(
                "spotify:track:{}",
                play_histories[idx].clone().track.id.unwrap()
            ));
        }
        uris
    }
    pub fn key_up(&mut self) {
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
    pub fn key_down(&mut self) {
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
}
