use std::io;

use termion::event::{Key, Event};
use termion::input::{MouseTerminal, TermRead};
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;
use tui::backend::TermionBackend;
use tui::layout::{Constraint, Direction, Layout};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Tabs, Widget};
use tui::Terminal;


pub struct SpotTermMenuTab {
    title: String,
    index: usize,
}


fn main() -> Result<(), Box<std::error::Error>> {
    // Terminal initialization
    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let stdout = AlternateScreen::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    // Main loop
    loop {
        terminal.draw(|mut f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(5)
                .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
                .split(size);

            Block::default()
                .style(Style::default().bg(Color::Black))
                .render(&mut f, size);
            Tabs::default()
                .block(Block::default().borders(Borders::ALL).title("Menu"))
                .titles(&vec!["Albums", "Artists"])
                .select(0)
                .style(Style::default().fg(Color::Cyan))
                .highlight_style(Style::default().fg(Color::Red))
                .render(&mut f, chunks[0]);

        })?;
        let stdin = io::stdin();
        let mut events = stdin.events();
        for event in events {
            let event = event.unwrap();
            match event {
                Event::Key(Key::Char('q')) => {
                    assert!(false);
                },
                _ => {}
            }
        }


    }
    Ok(())
}
