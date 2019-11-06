use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use termion::event::Key;
use termion::input::TermRead;

pub enum Event {
    KeyInput(Key),
    Tick,
    APIUpdate,
}

pub struct EventHandler {
    rx: mpsc::Receiver<Event>,
    input_handle: thread::JoinHandle<()>,
    tick_handle: thread::JoinHandle<()>,
    api_update_handle: thread::JoinHandle<()>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let input_handle = {
            let tx = tx.clone();
            thread::spawn(move || {
                let stdin = io::stdin();
                for evt in stdin.keys() {
                    match evt {
                        Ok(key) => {
                            if let Err(_) = tx.send(Event::KeyInput(key)) {
                                return;
                            }
                        }
                        Err(_) => {}
                    }
                }
            })
        };
        let tick_handle = {
            let tx = tx.clone();
            thread::spawn(move || {
                let tx = tx.clone();
                loop {
                    tx.send(Event::Tick).unwrap();
                    thread::sleep(Duration::from_millis(100));
                }
            })
        };
        let api_update_handle = {
            let tx = tx.clone();
            thread::spawn(move || {
                let tx = tx.clone();
                loop {
                    tx.send(Event::APIUpdate).unwrap();
                    thread::sleep(Duration::from_millis(1000));
                }
            })
        };

        EventHandler {
            rx: rx,
            input_handle: input_handle,
            tick_handle: tick_handle,
            api_update_handle: api_update_handle,
        }
    }
    pub fn next(&self) -> Result<Event, mpsc::RecvError> {
        self.rx.recv()
    }
}
