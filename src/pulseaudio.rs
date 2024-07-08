use symphonia::core::audio::*;
use symphonia::core::units::Duration;

use libpulse_binding as pulse;
use libpulse_simple_binding as psimple;

#[allow(dead_code)]
#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub enum AudioOutputError {
    OpenStreamError,
    PlayStreamError,
    StreamClosedError,
}

pub type Result<T> = std::result::Result<T, AudioOutputError>;
pub trait AudioOutput {
    fn write(&mut self, decoded: AudioBufferRef<'_>) -> Result<()>;
    fn flush(&mut self);
}

pub struct PulseAudioOutput {
    pa: psimple::Simple,
    sample_buf: RawSampleBuffer<f32>,
}

impl PulseAudioOutput {
    pub fn try_open(spec: SignalSpec, duration: Duration) -> Result<Box<dyn AudioOutput>> {
        // An interleaved buffer is required to send data to PulseAudio. Use a SampleBuffer to
        // move data between Symphonia AudioBuffers and the byte buffers required by PulseAudio.
        let sample_buf = RawSampleBuffer::<f32>::new(duration, spec);

        // Create a PulseAudio stream specification.
        let pa_spec = pulse::sample::Spec {
            format: pulse::sample::Format::FLOAT32NE,
            channels: spec.channels.count() as u8,
            rate: spec.rate,
        };

        assert!(pa_spec.is_valid());

        let pa_ch_map = map_channels_to_pa_channelmap(spec.channels);

        // Create a PulseAudio connection.
        let pa_result = psimple::Simple::new(
            None,                               // Use default server
            "Symphonia Player",                 // Application name
            pulse::stream::Direction::Playback, // Playback stream
            None,                               // Default playback device
            "Music",                            // Description of the stream
            &pa_spec,                           // Signal specification
            pa_ch_map.as_ref(),                 // Channel map
            None,                               // Custom buffering attributes
        );

        match pa_result {
            Ok(pa) => Ok(Box::new(PulseAudioOutput { pa, sample_buf })),
            Err(_err) => {
                Err(AudioOutputError::OpenStreamError)
            }
        }
    }
}

impl AudioOutput for PulseAudioOutput {
    fn write(&mut self, decoded: AudioBufferRef<'_>) -> Result<()> {
        // Do nothing if there are no audio frames.
        if decoded.frames() == 0 {
            return Ok(());
        }

        // Interleave samples from the audio buffer into the sample buffer.
        self.sample_buf.copy_interleaved_ref(decoded);

        // Write interleaved samples to PulseAudio.
        match self.pa.write(self.sample_buf.as_bytes()) {
            Err(_err) => {
                Err(AudioOutputError::StreamClosedError)
            }
            _ => Ok(()),
        }
    }

    fn flush(&mut self) {
        // Flush is best-effort, ignore the returned result.
        let _ = self.pa.drain();
    }
}

/// Maps a set of Symphonia `Channels` to a PulseAudio channel map.
fn map_channels_to_pa_channelmap(channels: Channels) -> Option<pulse::channelmap::Map> {
    let mut map: pulse::channelmap::Map = Default::default();
    map.init();
    map.set_len(channels.count() as u8);

    let is_mono = channels.count() == 1;

    for (i, channel) in channels.iter().enumerate() {
        map.get_mut()[i] = match channel {
            Channels::FRONT_LEFT if is_mono => pulse::channelmap::Position::Mono,
            Channels::FRONT_LEFT => pulse::channelmap::Position::FrontLeft,
            Channels::FRONT_RIGHT => pulse::channelmap::Position::FrontRight,
            Channels::FRONT_CENTRE => pulse::channelmap::Position::FrontCenter,
            Channels::REAR_LEFT => pulse::channelmap::Position::RearLeft,
            Channels::REAR_CENTRE => pulse::channelmap::Position::RearCenter,
            Channels::REAR_RIGHT => pulse::channelmap::Position::RearRight,
            Channels::LFE1 => pulse::channelmap::Position::Lfe,
            Channels::FRONT_LEFT_CENTRE => pulse::channelmap::Position::FrontLeftOfCenter,
            Channels::FRONT_RIGHT_CENTRE => pulse::channelmap::Position::FrontRightOfCenter,
            Channels::SIDE_LEFT => pulse::channelmap::Position::SideLeft,
            Channels::SIDE_RIGHT => pulse::channelmap::Position::SideRight,
            Channels::TOP_CENTRE => pulse::channelmap::Position::TopCenter,
            Channels::TOP_FRONT_LEFT => pulse::channelmap::Position::TopFrontLeft,
            Channels::TOP_FRONT_CENTRE => pulse::channelmap::Position::TopFrontCenter,
            Channels::TOP_FRONT_RIGHT => pulse::channelmap::Position::TopFrontRight,
            Channels::TOP_REAR_LEFT => pulse::channelmap::Position::TopRearLeft,
            Channels::TOP_REAR_CENTRE => pulse::channelmap::Position::TopRearCenter,
            Channels::TOP_REAR_RIGHT => pulse::channelmap::Position::TopRearRight,
            _ => {
                // If a Symphonia channel cannot map to a PulseAudio position then return None
                // because PulseAudio will not be able to open a stream with invalid channels.
                println!("failed to map channel {:?} to output", channel);
                return None;
            }
        }
    }

    Some(map)
}
