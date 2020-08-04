extern crate dirs;
extern crate log;
extern crate log4rs;
extern crate rpassword;
extern crate rspotify;
extern crate serde;
extern crate spoterm;
extern crate toml;

use std::fs;
use std::io;

use termion::event::Key;
use termion::input::MouseTerminal;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, List, Paragraph, Tabs, Text, Widget};
use tui::Terminal;

use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config;
use log4rs::config::Appender;
use log4rs::encode::pattern::PatternEncoder;
use spoterm::config::UserConfig;
use spoterm::event;
use spoterm::spoterm::SpotermClient;
use spoterm::spotify::SpotifyService;

//Authorization Scopes
//https://developer.spotify.com/documentation/general/guides/scopes/
pub const SCOPES: [&str; 18] = [
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

fn init_spoterm_config_if_needed() -> Result<(), failure::Error> {
    let config_dir = dirs::home_dir()
        .expect("can not find home directory")
        .join(".spoterm");
    //create a config dir ~/.spoterm/ if needed
    if !config_dir.exists() {
        fs::create_dir_all(config_dir.clone())?;
    }
    //create a config file ~/.spoterm/config.toml if needed
    let config = config_dir.join("config.toml");
    if !config.exists() {
        //read client id
        println!("config.toml not found and input your <CLIENT ID> and <CLIENT SECRET>");
        let client_id = rpassword::read_password_from_tty(Some("Client ID: "))?;
        let client_secret = rpassword::read_password_from_tty(Some("Client Secret: "))?;
        let user_config = UserConfig::new()
            .client_id(client_id)
            .client_secret(client_secret);
        fs::write(config.as_path(), toml::to_string(&user_config)?)?;
        //println!("Save your <CLIENT ID> and <CLIENT SECRET> in {}", config.as_os_str().to_os_string());
    }
    Ok(())
}

fn get_spotify_client_id_and_secret() -> Result<(String, String), Box<dyn std::error::Error>> {
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_spoterm_config_if_needed()?;

    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build("log/output.log")?;
    let config = config::Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(
            config::Root::builder()
                .appender("logfile")
                .build(LevelFilter::Info),
        )?;
    log4rs::init_config(config)?;

    let (client_id, client_secret) = get_spotify_client_id_and_secret()?;
    let spoterm_cache = dirs::home_dir()
        .expect("can not find home directory")
        .join(".spoterm")
        .join(".spotify_token_cache.json");
    let mut oauth = rspotify::oauth2::SpotifyOAuth::default()
        .scope(&SCOPES.join(" "))
        .client_id(&client_id)
        .client_secret(&client_secret)
        .redirect_uri("http://localhost:8888/callback")
        .cache_path(spoterm_cache)
        .build();

    let token_info = rspotify::util::get_token(&mut oauth).await.unwrap();

    let (tx, rx) = crossbeam::channel::unbounded();
    let spotify = SpotifyService::new(token_info, oauth).api_result_tx(tx.clone());
    let api_event_tx = spotify.api_event_tx.clone();
    let mut spoterm = SpotermClient::new(rx.clone(), api_event_tx.clone());

    spotify.run().await?;

    spoterm.request_device();
    spoterm.request_current_user_recently_played();
    spoterm.request_current_playback();
    spoterm.request_current_user_saved_tracks();
    spoterm.set_selected_device()?;
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let event_handler = event::EventHandler::new();
    loop {
        let content_ui = &mut spoterm.contents.uis[spoterm.selected_menu_tab_id];
        content_ui.set_data(&spoterm.spotify_data);
        content_ui.set_filter(spoterm.contents.filter.clone());
        if spoterm.contents.input_mode {
            match event_handler.next()? {
                event::Event::KeyInput(key) => match key {
                    Key::Char('\n') | Key::Esc => {
                        spoterm.contents.input_mode = false;
                    }
                    Key::Char(c) => {
                        spoterm.contents.filter.push(c);
                    }
                    Key::Backspace => {
                        spoterm.contents.filter.pop();
                    }
                    _ => {}
                },
                event::Event::Tick => {
                    spoterm.fetch_api_result();
                    spoterm.set_selected_device()?;
                }
                event::Event::APIUpdate => {
                    spoterm.request_device();
                    spoterm.request_current_playback();
                    spoterm.request_current_user_recently_played();
                    spoterm.request_current_user_saved_tracks();
                    spoterm.request_check_unknown_saved_tracks();
                } // _ => {}
            }
        } else {
            match event_handler.next()? {
                event::Event::KeyInput(key) => match key {
                    Key::Char('q') => {
                        break;
                    }
                    Key::Char('p') | Key::Char(' ') => {
                        spoterm.pause();
                        spoterm.request_current_playback();
                    }
                    Key::Char('/') => {
                        spoterm.contents.input_mode = true;
                    }
                    Key::Down | Key::Char('j') => {
                        content_ui.key_down();
                    }
                    Key::Up | Key::Char('k') => {
                        content_ui.key_up();
                    }
                    Key::Char('f') => {
                        spoterm.request_save_current_playback();
                    }
                    Key::Char('+') => {
                        spoterm.request_volume(true);
                    }
                    Key::Char('-') => {
                        spoterm.request_volume(false);
                    }
                    Key::Char('S') => {
                        spoterm.shuffle();
                        spoterm.request_current_playback();
                    }
                    Key::Char('r') => {
                        spoterm.request_repeat();
                    }
                    Key::Char('>') => {
                        spoterm.request_next_track();
                        spoterm.request_current_playback();
                    }
                    Key::Char('<') => {
                        spoterm.request_seek_to_zero_or_previous_track();
                        spoterm.request_current_playback();
                    }
                    Key::Char('\n') => {
                        content_ui.key_enter();
                    }
                    Key::Right | Key::Char('l') => {
                        spoterm.move_to_next_menu_tab();
                    }
                    Key::Left | Key::Char('h') => {
                        spoterm.move_to_previous_menu_tab();
                    }
                    _ => {}
                },
                event::Event::Tick => {
                    spoterm.fetch_api_result();
                    spoterm.set_selected_device()?;
                }
                event::Event::APIUpdate => {
                    spoterm.request_device();
                    spoterm.request_current_playback();
                    spoterm.request_current_user_recently_played();
                    spoterm.request_current_user_saved_tracks();
                    spoterm.request_check_unknown_saved_tracks();
                } // _ => {}
            }
        }
        let filter = spoterm.contents.filter.clone();
        terminal.draw(|mut f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(5)
                .constraints(
                    [
                        Constraint::Length(3),
                        Constraint::Length(5),
                        Constraint::Length(3),
                        Constraint::Length(3),
                    ]
                    .as_ref(),
                )
                .split(size);

            Tabs::default()
                .block(Block::default().borders(Borders::ALL).title("Menu"))
                .titles(&spoterm.menu_tabs)
                .select(spoterm.selected_menu_tab_id)
                .style(Style::default().fg(Color::Cyan))
                .highlight_style(Style::default().fg(Color::Red))
                .render(&mut f, chunks[0]);
            List::new(spoterm.player_items().into_iter())
                .block(Block::default().borders(Borders::ALL).title("Player"))
                .render(&mut f, chunks[1]);

            let filter_title = if spoterm.contents.input_mode {
                "Filter(Entering.... Quit: Enter)"
            } else {
                "Filter(Filter Mode: /)"
            };
            Paragraph::new([Text::raw(filter)].iter())
                .style(Style::default().fg(Color::White))
                .block(Block::default().borders(Borders::ALL).title(filter_title))
                .render(&mut f, chunks[2]);

            spoterm.contents.uis[spoterm.selected_menu_tab_id].render(&mut f, chunks[3]);
        })?;
    }
    Ok(())
}
