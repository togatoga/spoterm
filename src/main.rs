extern crate dirs;
#[macro_use]
extern crate log;
extern crate log4rs;
extern crate serde;
extern crate spoterm;
extern crate toml;
extern crate rpassword;
extern crate rspotify;

use std::fs;
use std::io;
use std::path;

use termion::event::{Event, Key};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, SelectableList, Tabs, Widget};
use tui::Terminal;

use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config;
use log4rs::config::Appender;
use log4rs::encode::pattern::PatternEncoder;
use spoterm::config::UserConfig;
use spoterm::event;
use spoterm::spotify::SpotifyClient;


fn init_spoterm_config_if_needed() -> Result<(), failure::Error> {

    let config_dir = dirs::home_dir()
        .expect("can not find home directory")
        .join(".spoterm");
    //create a config dir ~/.spoterm/ if needed
    if !config_dir.exists(){
        fs::create_dir_all(config_dir.clone())?;
    }
    //create a config file ~/.spoterm/config.toml if needed
    let config = config_dir.join("config.toml");
    if !config.exists() {
        //read client id
        println!("config.toml not found and input your <CLIENT ID> and <CLIENT SECRET>");
        let client_id = rpassword::read_password_from_tty(Some("Client ID: "))?;
        let client_secret = rpassword::read_password_from_tty(Some("Client Secret: "))?;
        let user_config = UserConfig::new().client_id(client_id).client_secret(client_secret);
        fs::write(config.as_path(), toml::to_string(&user_config)?)?;
        //println!("Save your <CLIENT ID> and <CLIENT SECRET> in {}", config.as_os_str().to_os_string());
    }
    Ok(())
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
    let mut spotify_client = SpotifyClient::new(client_id, client_secret);
    spotify_client.fetch_device()?;
    if let Err(e) = spotify_client.fetch_recent_play_history() {
        info!("{}", e);
    }
    //spotify_client.spotify.clone().start_playback()
    let mut play_histories = vec![];
    for history in spotify_client
        .recent_played
        .recent_play_histories
        .clone()
        .unwrap()
        .iter()
    {
        info!(
            "Song: {} Artist: {} ID: {}",
            history.track.name,
            history.track.artists[0].name,
            history.track.id.clone().unwrap()
        );
        play_histories.push(format!(
            "{} - {}",
            history.track.name, history.track.artists[0].name
        ));
    }

    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;
    let device_id = spotify_client.selected_device.as_ref().unwrap().id.clone();

    let mut event_handler = event::EventHandler::new();

    // Main loop
    loop {
        let device_id = spotify_client.selected_device.as_ref().unwrap().id.clone();
        terminal.draw(|mut f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(5)
                .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
                .split(size);

            Tabs::default()
                .block(Block::default().borders(Borders::ALL).title("Menu"))
                .titles(&vec!["Recently Played", "Albums", "Artists"])
                .select(0)
                .style(Style::default().fg(Color::Cyan))
                .highlight_style(Style::default().fg(Color::Red))
                .render(&mut f, chunks[0]);

            spotify_client.recent_played.render(&mut f, chunks[1]);

        })?;

        match event_handler.next()? {
            event::Event::KeyInput(key) => match key {
                Key::Char('q') => {
                    break;
                }
                Key::Down => {
                    spotify_client.recent_played.key_down();
                }
                Key::Up => {
                    spotify_client.recent_played.key_up();
                }
                Key::Char('\n') => {

                    let uris = spotify_client.recent_played.key_enter();
                    spotify_client.spotify.clone().start_playback(
                        Some(device_id),
                        None,
                        Some(uris),
                        None,
                    )?;
                    info!("Play Music!!");
                    //info!("Play Music!! {:?}", uris);
                    //spotify_client.spotify.start_playback(device_id);
                }
                Key::Char('p') => {

                    if let Some(current_play_back) = spotify_client.spotify.clone().current_playback(None)? {
                        if current_play_back.is_playing {
                            //pause
                            spotify_client
                                .spotify
                                .clone()
                                .pause_playback(Some(spotify_client.selected_device.clone().unwrap().id))?;
                        } else {
                            //unpause
                            spotify_client.spotify.clone().start_playback(Some(device_id.clone()), None, None, None)?;
                        }
                    }

                },
                Key::Char('+') => {
                    let device_id = spotify_client.selected_device.clone().unwrap().id;
                    let volume_percent = spotify_client.selected_device.clone().unwrap().volume_percent as u8;
                    if volume_percent + 5 <= 100 {
                        spotify_client.spotify.volume(volume_percent + 5, Some(device_id));
                        info!("Volume up!! {}", volume_percent + 5);
                        spotify_client.fetch_device()?;
                    }
                },
                Key::Char('-') => {
                    let device_id = spotify_client.selected_device.clone().unwrap().id;
                    let volume_percent = spotify_client.selected_device.clone().unwrap().volume_percent as u8;
                    if volume_percent > 0 {
                        spotify_client.spotify.volume(volume_percent - 5, Some(device_id));
                        info!("Volume Down!! {}", volume_percent - 5);
                        spotify_client.fetch_device()?;
                    }
                },
                _ => {}
            },
            event::Event::Tick => {
                spotify_client.fetch_recent_play_history()?;
            },
            _ => {}
        }
    }
    Ok(())
}
