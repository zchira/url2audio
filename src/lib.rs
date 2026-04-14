mod cpalaudio;
pub mod player_engine;
mod resampler;
mod url_source;
mod url_source_buff;

use std::sync::{Arc, RwLock};

#[derive(thiserror::Error, Debug)]
pub enum Url2AudioError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] ureq::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("No content-length header")]
    NoContentLength,
}

use crossbeam_channel::{unbounded, Receiver, Sender};
use player_engine::Playing;

use crate::player_engine::{PlayerActions, PlayerEngine, PlayerState, PlayerStatus};

/// Main Player struct. Wrapper around `player_engine`.
pub struct Player {
    inner_player: Arc<RwLock<PlayerEngine>>,
    tx: Sender<PlayerActions>,
    rx_status: Receiver<PlayerStatus>,
    state: Arc<RwLock<PlayerState>>,
    events_rx: Receiver<PlayerStatus>,
}

impl Player {
    /// Create new instance of Player. Initialized and ready to use.
    /// On creation inner_thread is started and ready for receiving engine's messages.
    /// All player methods are fire-and-forget. They are non-blocking but the execution of
    /// action will not happend immediately.
    /// Example:
    /// When `player.pause()` the command message for pausing will be sent, and it will be 
    /// executed in player_engine's thread's next loop.
    pub fn new() -> Self {
        let (tx, rx) = unbounded();
        let (tx_status, rx_status) = unbounded();
        let (tx_events, rx_events) = unbounded();
        let mut to_ret = Player {
            inner_player: Arc::new(RwLock::new(PlayerEngine::new(
                rx.clone(),
                tx_status.clone(),
            ))),
            tx,
            rx_status,
            state: Arc::new(RwLock::new(PlayerState {
                playing: Playing::Playing,
                duration: 0.0,
                position: 0.0,
                pending_seek: None,
                error: None,
                chunks: Default::default(),
            })),
            events_rx: rx_events,
        };
        to_ret.inner_thread(tx_events);
        to_ret
    }

    /// Open stream from provided url (`src`). Playback will start immediately.
    pub fn open(&mut self, src: &str) {
        let _ = self.tx.send(PlayerActions::Open(src.to_string()));
    }

    fn inner_thread(&mut self, tx_events: Sender<PlayerStatus>) {
        let player = self.inner_player.clone();

        let _ = std::thread::spawn(move || {
            let mut p = player.write().unwrap();
            let _result = p.start();
        });

        let rx1 = self.rx_status.clone();
        let s = self.state.clone();
        let _ = std::thread::spawn(move || loop {
            match rx1.recv() {
                Ok(a) => {
                    let mut state = s.write().unwrap();
                    match a {
                        PlayerStatus::SendPlaying(ref playing) => {
                            state.playing = playing.clone();
                            let _ = tx_events.send(a);
                        }
                        PlayerStatus::SendTimeStats(position, duration) => {
                            state.duration = duration;
                            if state.pending_seek.is_none() {
                                state.position = position;
                            }
                        }
                        PlayerStatus::Error(ref err) => {
                            if state.position - state.duration >= -1.0 {
                                state.error = None;
                                state.playing = Playing::Finished;
                            } else {
                                state.error = Some(err.clone());
                            }
                            let _ = tx_events.send(a);
                        }
                        PlayerStatus::ClearError => {
                            state.error = None;
                            state.chunks = Default::default();
                        }
                        PlayerStatus::ChunkAdded(start, end) => {
                            state.chunks.push((start, end));
                        },
                        PlayerStatus::Seeked(t) => {
                            state.pending_seek = None;
                            state.position = t;
                            let _ = tx_events.send(PlayerStatus::Seeked(t));
                        },
                        PlayerStatus::Opened(_) | PlayerStatus::Closed => {
                            let _ = tx_events.send(a);
                        },
                    }
                }
                Err(_) => break,
            }
        });
    }

    /// Start playback (if paused)
    pub fn play(&self) {
        let _ = self.tx.send(PlayerActions::Resume);
    }

    /// Pause playback.
    pub fn pause(&self) {
        let _ = self.tx.send(PlayerActions::Pause);
    }

    /// Close opened stream.
    pub fn close(&self) {
        let _ = self.tx.send(PlayerActions::Close);
    }

    /// Is player in Playing state.
    pub fn is_playing(&self) -> Playing {
        self.state.read().unwrap().playing.clone()
    }

    /// Return description of buffered chunks.
    /// Every element of vec contains start and end position of chunk.
    /// Values are normalized to range 0.0 - 1.0
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
        let new_pos = (self.current_position() + dt).max(0.0);
        self.state.write().unwrap().pending_seek = Some(new_pos);
        let _ = self.tx.send(PlayerActions::Seek(new_pos));
    }

    /// Current playback position
    pub fn current_position(&self) -> f64 {
        let state = self.state.read().unwrap();
        state.pending_seek.unwrap_or(state.position)
    }

    /// Duration in seconds
    pub fn duration(&self) -> f64 {
        self.state.read().unwrap().duration
    }

    /// Indicator if player is in error state.
    pub fn is_in_error_state(&self) -> bool {
        self.state.read().unwrap().error.is_some()
    }

    /// Current error message (if any)
    pub fn error(&self) -> Option<String> {
        self.state.read().unwrap().error.clone()
    }

    /// Receiver for player events (action feedback, errors, play state changes).
    /// Use `recv()` or `try_recv()` to consume events.
    pub fn events(&self) -> &Receiver<PlayerStatus> {
        &self.events_rx
    }

    /// User friendly display of current tima
    pub fn current_position_display(&self) -> String {
        self.time_to_display(self.current_position())
    }

    /// User friendly display of duration
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
