use core::borrow::BorrowMut;
use rspotify::spotify::model::playing::PlayHistory;
use tui::widgets::{Block, Borders, SelectableList, Tabs, Widget};

#[derive(Clone, Debug)]
pub struct RecentPlayed {
    pub selected_id: Option<usize>,
    pub recent_play_histories: Option<Vec<PlayHistory>>,
}

impl RecentPlayed {
    pub fn new() -> RecentPlayed {
        RecentPlayed {
            selected_id: None,
            recent_play_histories: None,
        }
    }

    pub fn create_view(&self) -> SelectableList {
        SelectableList::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Recently Played"),
            )
            .select(self.selected_id)
            .highlight_symbol(">")
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