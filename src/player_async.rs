#![cfg(feature="async")]
use std::{
    sync::Arc,
    thread::sleep,
    time::Duration,
};

use tokio::sync::RwLock;

use crossbeam_channel::{unbounded, Receiver, Sender};
use crate::player_engine::Playing;

use crate::player_engine::{PlayerActions, PlayerEngine, PlayerState, PlayerStatus};

/// Main Player struct. Wrapper around `player_engine`.
#[cfg(feature = "async")]
pub struct PlayerAsync {
    inner_player: Arc<RwLock<PlayerEngine>>,
    tx: Sender<PlayerActions>,
    rx_status: Receiver<PlayerStatus>,
    state: Arc<RwLock<PlayerState>>,
}

#[cfg(feature = "async")]
impl PlayerAsync {
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
        let mut to_ret = PlayerAsync {
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

    /// Open stream from provided url (`src`). Playback will start immediately.
    pub fn open(&mut self, src: &str) {
        let _ = self.tx.send(PlayerActions::Open(src.to_string()));
    }

    fn inner_thread(&mut self) {
        let player = self.inner_player.clone();

        // let _ = self.tx.send(PlayerActions::Close);
        let _ = tokio::spawn(async move {
            let mut p = player.write().await;
            let _result = p.start(); //(&path);
        });

        let rx1 = self.rx_status.clone();
        let s = self.state.clone();
        let _ = tokio::spawn(async move {
            loop {
                let a = rx1.try_recv();

                match a {
                    Ok(a) => {
                        let mut state = s.write().await;
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
    pub async fn is_playing(&self) -> Playing {
        self.state.read().await.playing.clone()
    }

    /// Return description of buffered chunks.
    /// Every element of vec contains start and end position of chunk.
    /// Values are normalized to range 0.0 - 1.0
    pub async fn buffer_chunks(&self) -> Vec<(f32, f32)> {
        self.state.read().await.chunks.clone()
    }

    /// seek to time from the beginning.
    /// `time` is in seconds
    pub fn seek(&self, time: f64) {
        let _ = self.tx.send(PlayerActions::Seek(time));
    }

    /// seek to time relative from current position
    pub async fn seek_relative(&self, dt: f64) {
        let new_pos = self.current_position().await + dt;
        let _ = self.tx.send(PlayerActions::Seek(new_pos));
    }

    /// Current playback position
    pub async fn current_position(&self) -> f64 {
        self.state.read().await.position
    }

    /// Duration in seconds
    pub async fn duration(&self) -> f64 {
        self.state.read().await.duration
    }

    /// Indicator if player is in error state.
    pub async fn is_in_error_state(&self) -> bool {
        self.state.read().await.error.is_some()
    }

    /// Current error message (if any)
    pub async fn error(&self) -> Option<String> {
        self.state.read().await.error.clone()
    }

    /// User friendly display of current tima
    pub async fn current_position_display(&self) -> String {
        self.time_to_display(self.current_position().await)
    }

    /// User friendly display of duration
    pub async fn duration_display(&self) -> String {
        self.time_to_display(self.duration().await)
    }

    fn time_to_display(&self, seconds: f64) -> String {
        let is: i64 = seconds.round() as i64;
        let hours = is / (60 * 60);
        let mins = (is % (60 * 60)) / 60;
        let secs = seconds - 60.0 * mins as f64 - 60.0 * 60.0 * hours as f64; // is % 60;
        format!("{}:{:0>2}:{:0>4.1}", hours, mins, secs)
    }
}
