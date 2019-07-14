extern crate dirs;
extern crate serde;
extern crate spoterm;
extern crate toml;

use std::fs;
use std::io;

use termion::event::{Event, Key};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Tabs, Widget};
use tui::Terminal;

use spoterm::config::UserConfig;
use spoterm::event;
use spoterm::spotify::SpotifyClient;

pub struct SpotTermMenuTab {
    title: String,
    index: usize,
}

fn get_spotify_client_id_and_secret() -> Result<(String, String), Box<std::error::Error>> {
    //read config from file
    let config = dirs::home_dir()
        .expect("can not find home directory")
        .join(".spoterm")
        .join("config.toml");
    let config_content = fs::read_to_string(config.to_str().expect("can not read config file"))?;
    let user_config: UserConfig = toml::from_str(&config_content)?;

    Ok((
        user_config.profile.client_id,
        user_config.profile.client_secret,
    ))
}

fn main() -> Result<(), Box<std::error::Error>> {
    let (client_id, client_secret) = get_spotify_client_id_and_secret()?;
    let mut spotify_client = SpotifyClient::new(client_id, client_secret);
    spotify_client.fetch_recent_play_history();
    for history in spotify_client.recent_play_history.clone().unwrap().iter() {
        println!("Song: {} Artist: {}", history.track.name, history.track.artists[0].name);
    }
    println!("{}", spotify_client.recent_play_history.is_some());

    assert!(false);
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let mut event_handler = event::EventHandler::new();
    // Main loop
    loop {
        terminal.draw(|mut f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(5)
                .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
                .split(size);

            /*Block::default()
            .style(Style::default().bg(Color::Black))
            .render(&mut f, size);*/
            Tabs::default()
                .block(Block::default().borders(Borders::ALL).title("Menu"))
                .titles(&vec!["Recently Played", "Albums", "Artists"])
                .select(0)
                .style(Style::default().fg(Color::Cyan))
                .highlight_style(Style::default().fg(Color::Red))
                .render(&mut f, chunks[0]);
        })?;
        match event_handler.next()? {
            event::Event::KeyInput(key) => match key {
                Key::Char('q') => {
                    break;
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(())
}
