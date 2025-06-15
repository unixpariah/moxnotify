use std::{collections::VecDeque, fs, path::Path, thread, time::Duration};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};
use tinyaudio::{run_output_device, OutputDeviceParameters};

#[derive(Clone)]
pub struct Playback {
    duration: Duration,
    buffer: VecDeque<f32>,
    params: OutputDeviceParameters,
    shutdown_channel: Option<(
        crossbeam_channel::Sender<()>,
        crossbeam_channel::Receiver<()>,
    )>,
}

impl Playback {
    pub fn new<T>(path: T) -> anyhow::Result<Self>
    where
        T: AsRef<Path>,
    {
        let src = fs::File::open(&path)?;
        let mss = MediaSourceStream::new(Box::new(src), Default::default());
        let hint = Hint::new();

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| anyhow::anyhow!("Failed to probe audio format: {}", e))?;

        let track = probed
            .format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(anyhow::anyhow!("No valid audio track found"))?;

        let channels_count = track
            .codec_params
            .channels
            .map(|channels| channels.count())
            .ok_or(anyhow::anyhow!("Unable to determine channel count"))?;

        let sample_rate = track
            .codec_params
            .sample_rate
            .ok_or(anyhow::anyhow!("Unable to determine sample rate"))?
            as usize;

        let mut format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(anyhow::anyhow!(""))?;

        let dec_opts = DecoderOptions::default();
        let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

        let mut audio_buffer: VecDeque<f32> = VecDeque::new();

        let duration = if let Some(time_base) = track.codec_params.time_base {
            if let Some(n_frames) = track.codec_params.n_frames {
                let duration_seconds =
                    (n_frames as f64) / (time_base.denom as f64 / time_base.numer as f64);
                Some(std::time::Duration::from_secs_f64(duration_seconds))
            } else {
                None
            }
        } else {
            None
        }
        .unwrap();

        let track_id = track.id;
        while let Ok(packet) = format.next_packet() {
            while !format.metadata().is_latest() {
                format.metadata().pop();
            }
            if packet.track_id() != track_id {
                continue;
            }
            let decoded = decoder.decode(&packet)?;
            let mut sample_buf =
                SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
            sample_buf.copy_interleaved_ref(decoded);
            let samples: &[f32] = bytemuck::cast_slice(sample_buf.samples());

            {
                let buffer = &mut audio_buffer;
                samples.iter().for_each(|sample| {
                    buffer.push_back(*sample);
                });
            }
        }

        let params = OutputDeviceParameters {
            channels_count,
            sample_rate,
            channel_sample_count: audio_buffer.len(),
        };

        Ok(Self {
            duration,
            buffer: audio_buffer,
            params,
            shutdown_channel: None,
        })
    }

    pub fn start(&mut self) {
        self.shutdown_channel = Some(crossbeam_channel::unbounded());

        let mut buffer = self.buffer.clone();
        let params = self.params;
        let duration = self.duration;

        if let Some(channel) = self.shutdown_channel.as_ref() {
            let rx = channel.1.clone();
            thread::spawn(move || {
                let _device = run_output_device(params, move |data| {
                    data.iter_mut().for_each(|sample| {
                        if let Some(audio_sample) = buffer.pop_front() {
                            *sample = audio_sample;
                        } else {
                            *sample = 0.0;
                        }
                    });
                })
                .unwrap();

                _ = rx.recv_timeout(duration);
            });
        }
    }

    pub fn stop(&mut self) {
        if let Some(channel) = self.shutdown_channel.as_ref() {
            _ = channel.0.send(());
        }
    }
}
