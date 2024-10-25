mod cpalaudio;
pub mod player_engine;
mod resampler;
mod url_source;
mod url_source_buff;

use std::{
    sync::{Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use player_engine::Playing;

use crate::player_engine::{PlayerActions, PlayerEngine, PlayerState, PlayerStatus};

pub struct Player {
    inner_player: Arc<RwLock<PlayerEngine>>,
    tx: Sender<PlayerActions>,
    // _rx: Receiver<PlayerActions>,
    rx_status: Receiver<PlayerStatus>,
    // _tx_status: Sender<PlayerStatus>,
    state: Arc<RwLock<PlayerState>>,
}

impl Player {
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        let (tx_status, rx_status) = unbounded();
        let mut to_ret = Player {
            inner_player: Arc::new(RwLock::new(PlayerEngine::new(
                rx.clone(),
                tx_status.clone(),
            ))),
            tx,
            // rx,
            rx_status,
            // tx_status,
            state: Arc::new(RwLock::new(PlayerState {
                playing: Playing::Playing,
                duration: 0.0,
                position: 0.0,
                error: None,
                chunks: Default::default()
            })),
        };
        to_ret.inner_thread();
        to_ret
    }

    pub fn open(&mut self, src: &str) {
        let _ = self.tx.send(PlayerActions::Open(src.to_string()));
    }

    pub fn inner_thread(&mut self) {
        let player = self.inner_player.clone();

        // let _ = self.tx.send(PlayerActions::Close);
        let _ = std::thread::spawn(move || {
            let mut p = player.write().unwrap();
            let _result = p.start(); //(&path);
        });

        let rx1 = self.rx_status.clone();
        let s = self.state.clone();
        let _ = std::thread::spawn(move || loop {
            let a = rx1.try_recv();

            match a {
                Ok(a) => {
                    let mut state = s.write().unwrap();
                    match a {
                        PlayerStatus::SendPlaying(playing) => {
                            state.playing = playing;
                        }
                        PlayerStatus::SendTimeStats(position, duration) => {
                            state.position = position;
                            state.duration = duration;
                        }
                        PlayerStatus::Error(err) => {
                            if state.position - state.duration >= -1.0 {
                                state.error = None;
                                state.playing = Playing::Finished;
                            } else {
                                state.error = Some(err);
                            }
                        }
                        PlayerStatus::ClearError => {
                            state.error = None;
                            state.chunks = Default::default();

                        }
                        PlayerStatus::ChunkAdded(start, end) => {
                            state.chunks.push((start, end));
                        },
                    }
                }
                Err(_) => {}
            }
            sleep(Duration::from_millis(10));
        });
    }

    pub fn play(&self) {
        let _ = self.tx.send(PlayerActions::Resume);
    }

    pub fn pause(&self) {
        let _ = self.tx.send(PlayerActions::Pause);
    }

    pub fn close(&self) {
        let _ = self.tx.send(PlayerActions::Close);
    }

    pub fn toggle_play(&self) {}

    pub fn is_playing(&self) -> Playing {
        self.state.read().unwrap().playing.clone()
    }

    pub fn buffer_chunks(&self) -> Vec<(f32, f32)> {
        self.state.read().unwrap().chunks.clone()
    }

    /// seek to time from the beginning.
    /// `time` is in seconds
    pub fn seek(&self, time: f64) {
        let _ = self.tx.send(PlayerActions::Seek(time));
    }

    /// seek to time relative from current position
    pub fn seek_relative(&self, dt: f64) {
        let new_pos = self.current_position() + dt;
        let _ = self.tx.send(PlayerActions::Seek(new_pos));
    }

    pub fn current_position(&self) -> f64 {
        self.state.read().unwrap().position
    }

    pub fn duration(&self) -> f64 {
        self.state.read().unwrap().duration
    }

    pub fn is_in_error_state(&self) -> bool {
        self.state.read().unwrap().error.is_some()
    }

    pub fn error(&self) -> Option<String> {
        self.state.read().unwrap().error.clone()
    }

    pub fn current_position_display(&self) -> String {
        self.time_to_display(self.current_position())
    }

    pub fn duration_display(&self) -> String {
        self.time_to_display(self.duration())
    }

    fn time_to_display(&self, seconds: f64) -> String {
        let is: i64 = seconds.round() as i64;
        let hours = is / (60 * 60);
        let mins = (is % (60 * 60)) / 60;
        let secs = seconds - 60.0 * mins as f64 - 60.0 * 60.0 * hours as f64; // is % 60;
        format!("{}:{:0>2}:{:0>4.1}", hours, mins, secs)
    }
}
