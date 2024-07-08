use crossbeam_channel::{Receiver, Sender};
use symphonia::core::units::TimeBase;
use std::thread::sleep;
use symphonia::core::codecs::{Decoder, CODEC_TYPE_NULL};
use symphonia::core::{
    audio::SignalSpec,
    codecs::DecoderOptions,
    errors::{Error, Result},
    formats::{FormatOptions, FormatReader, SeekMode, SeekTo, Track},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
    units::{Duration, Time},
};

use crate::{
    pulseaudio::{self, AudioOutput, PulseAudioOutput},
    url_source::UrlSource,
};

#[derive(PartialEq, Clone, Debug)]
pub enum PlayerActions {
    Pause,
    Resume,
    Seek(f64),
    Close,
    Open(String) // veliki loop za sve (open, play, play track...)
}

#[derive(PartialEq, Clone, Debug)]
pub enum PlayerStatus {
    SendPlaying(bool),
    SendDuration(f64),
    SendPosition(f64),
}

pub struct PlayerEngine {
    reader: Option<Box<dyn FormatReader>>,
    rx: Receiver<PlayerActions>,
    tx: Sender<PlayerActions>,
    // rx_status: Receiver<PlayerStatus>,
    tx_status: Sender<PlayerStatus>,
    src: Option<String>,
    initiate_drop: bool
}

#[derive(Clone, Debug)]
pub struct PlayerState {
    pub playing: bool,
    pub duration: f64,
    pub position: f64,
}

impl PlayerEngine {
    pub fn new(
        tx: Sender<PlayerActions>,
        rx: Receiver<PlayerActions>,
        tx_status: Sender<PlayerStatus>,
        // rx_status: Receiver<PlayerStatus>
    ) -> Self {
        Self {
            reader: None,
            rx,
            tx,
            tx_status,
            // rx_status
            src: None,
            initiate_drop: false
        }
    }

    pub fn start(&mut self) -> Result<i32> {
        let mut playing = true;
        let mut track_id: u32 = 0;
        let mut track: Option<&Track>;
        let mut tb: Option<TimeBase> = None;
        let mut dur: Option<u64> = None;
        let mut decoder: Option<Box<dyn Decoder>> = None;
        let mut audio_output = None;
        let decode_opts: DecoderOptions = Default::default();
        let result = loop {
            let action = match self.rx.try_recv() {
                Ok(a) => {
                    Some(a)
                }
                Err(_e) => None,
            };

            if let Some(ref a) = action {
                match a {
                    PlayerActions::Close => todo!(),
                    PlayerActions::Open(src) => {
                        let _res = self.open(&src);
                        let track_num: Option<usize> = None;

                        if let Some(reader) = self.reader.as_mut() {
                            track = track_num
                                .and_then(|t| reader.tracks().get(t))
                                .or_else(|| first_supported_track(reader.tracks()));

                            track_id = track.unwrap().id;

                            (tb, dur, decoder) = if let Some(r) = self.reader.as_mut() {
                                let track = match r.tracks().iter().find(|track| track.id == track_id) {
                                    Some(track) => track,
                                    _ => return Err(symphonia::core::errors::Error::IoError(
                                            std::io::Error::new(std::io::ErrorKind::Other, ""),
                                    )),
                                };

                                // Create a decoder for the track.
                                let dec = symphonia::default::get_codecs().make(&track.codec_params, &decode_opts)?;

                                let tb = track.codec_params.time_base;
                                let dur = track
                                    .codec_params
                                    .n_frames
                                    .map(|frames| track.codec_params.start_ts + frames);
                                (tb, dur, Some(dec))
                            } else {
                                return Err(symphonia::core::errors::Error::IoError(
                                        std::io::Error::new(std::io::ErrorKind::Other, ""),
                                ));
                            };
                        }
                    }
                    _ => {},
                }
            }

            if self.reader.is_none() {
                sleep(std::time::Duration::from_millis(200));
                continue;
            }

            ///// play_track

            let a = action.clone();
            if a.is_some() && (a.unwrap() == PlayerActions::Pause) {
                playing = false;
                let s = self.tx_status.send(PlayerStatus::SendPlaying(false));
            }

            let a = action.clone();
            if a.is_some() && (a.unwrap() == PlayerActions::Resume) {
                playing = true;
                let s = self.tx_status.send(PlayerStatus::SendPlaying(true));
            }

            {
                if !playing {
                    sleep(std::time::Duration::from_millis(200));
                    continue;
                }
            }

            let packet = if let Some(reader) = self.reader.as_mut() {
                match reader.next_packet() {
                    Ok(packet) => packet,
                    Err(err) => break Err(err),
                }
            } else {
                break Err(symphonia::core::errors::Error::IoError(
                        std::io::Error::new(std::io::ErrorKind::Other, "")));
            };

            if packet.track_id() != track_id {
                continue;
            }

            if let Some(ref mut decoder) = decoder {
                match decoder.decode(&packet) {
                    Ok(decoded) => {
                        if audio_output.is_none() {
                            let spec = *decoded.spec();
                            let duration = decoded.capacity() as u64;
                            audio_output.replace(try_open(spec, duration).unwrap());
                        } else {
                            // TODO: Check the audio spec. and duration hasn't changed.
                        }

                        let ts = packet.ts();
                        let (position, duration) = update_progress(ts, dur, tb);
                        {
                            let _ = self.tx_status.send(PlayerStatus::SendDuration(duration));
                            let _ = self.tx_status.send(PlayerStatus::SendPosition(position));
                        }

                        if let Some(ref mut audio_output) = audio_output {
                            audio_output.write(decoded).unwrap()
                        }

                        let a = action.clone();
                        if a.is_some() {
                            let a = a.as_ref().unwrap();
                            match a {
                                PlayerActions::Seek(ref t) => {
                                    let ts: Time = t.clone().into(); // packet.ts() + 30;
                                    if let Some(reader) = self.reader.as_mut() {
                                        let r = reader.seek(
                                            SeekMode::Accurate,
                                            SeekTo::Time {
                                                time: ts,
                                                track_id: Some(0),
                                            },
                                        );
                                    }
                                }
                                _ => {}
                            }
                        }
                    }

                    Err(Error::DecodeError(err)) => {
                        // Decode errors are not fatal. Print the error message and try to decode the next
                        // packet as usual.
                        println!("decode error: {}", err);
                    }
                    Err(err) => break Err(err),
                };
            }
            sleep(std::time::Duration::from_millis(20));
            ////
        };
        result
    }

    fn open(&mut self, path: &str) -> Result<i32> {
        let r = UrlSource::new(path);
        // let source = Box::new(ReadOnlySource::new(r));
        let source = Box::new(r);

        let hint = Hint::new();
        let mss = MediaSourceStream::new(source, Default::default());

        let format_opts = FormatOptions {
            enable_gapless: true,
            ..Default::default()
        };
        let metadata_opts: MetadataOptions = Default::default();

        match symphonia::default::get_probe().format(&hint, mss, &format_opts, &metadata_opts) {
            Ok(probed) => {
                self.reader = Some(probed.format);
                Ok(0)
            }
            Err(e) => {
                println!("input not supported: {:#?}", e);
                Err(e)
            }
        }
    }

    fn print_progress(
        &mut self,
        ts: u64,
        dur: Option<u64>,
        tb: Option<symphonia::core::formats::prelude::TimeBase>,
    ) {
        if let Some(tb) = tb {
            let t = tb.calc_time(ts);

            let hours = t.seconds / (60 * 60);
            let mins = (t.seconds % (60 * 60)) / 60;
            let secs = f64::from((t.seconds % 60) as u32) + t.frac;

            println!("\r\u{25b6}\u{fe0f}  {}:{:0>2}:{:0>4.1}", hours, mins, secs);

            let d = tb.calc_time(dur.unwrap_or(0));

            let hours = d.seconds / (60 * 60);
            let mins = (d.seconds % (60 * 60)) / 60;
            let secs = f64::from((d.seconds % 60) as u32) + d.frac;

            println!("::::> {}:{:0>2}:{:0>4.1}", hours, mins, secs);
        }
    }
}

impl Drop for PlayerEngine {
    fn drop(&mut self) {
        println!("DROP");
        self.initiate_drop = true;
    }
}

fn ignore_end_of_stream_error(result: Result<()>) -> Result<()> {
    match result {
        Err(Error::IoError(err))
            if err.kind() == std::io::ErrorKind::UnexpectedEof
                && err.to_string() == "end of stream" =>
        {
            // Do not treat "end of stream" as a fatal error. It's the currently only way a
            // format reader can indicate the media is complete.
            Ok(())
        }
        _ => result,
    }
}

pub fn try_open(spec: SignalSpec, duration: Duration) -> pulseaudio::Result<Box<dyn AudioOutput>> {
    PulseAudioOutput::try_open(spec, duration)
}

fn update_progress(
    ts: u64,
    dur: Option<u64>,
    tb: Option<symphonia::core::formats::prelude::TimeBase>,
) -> (f64, f64) {
    if let Some(tb) = tb {
        let t = tb.calc_time(ts);
        let position = t.frac + t.seconds as f64;

        let d = tb.calc_time(dur.unwrap_or(0));
        let duration = d.frac + d.seconds as f64;

        (position, duration)
    } else {
        (0.0, 0.0)
    }
}

fn first_supported_track(tracks: &[Track]) -> Option<&Track> {
    tracks
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
}
