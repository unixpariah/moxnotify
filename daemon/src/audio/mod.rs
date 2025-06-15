use std::{collections::VecDeque, fs, path::Path, thread};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};
use tinyaudio::{run_output_device, OutputDeviceParameters};

pub struct Audio {
    muted: bool,
}

impl Audio {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self { muted: false })
    }

    pub fn play<T>(&mut self, path: T) -> anyhow::Result<()>
    where
        T: AsRef<Path>,
    {
        if self.muted {
            return Ok(());
        }

        let src = fs::File::open(&path)?;
        thread::spawn(move || {
            let mss = MediaSourceStream::new(Box::new(src), Default::default());
            let hint = Hint::new();

            let probed = symphonia::default::get_probe()
                .format(
                    &hint,
                    mss,
                    &FormatOptions::default(),
                    &MetadataOptions::default(),
                )
                .map_err(|e| anyhow::anyhow!("Failed to probe audio format: {}", e))
                .unwrap();

            let track = probed
                .format
                .tracks()
                .iter()
                .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                .ok_or(anyhow::anyhow!("No valid audio track found"))
                .unwrap();

            let channels_count = track
                .codec_params
                .channels
                .map(|channels| channels.count())
                .ok_or(anyhow::anyhow!("Unable to determine channel count"))
                .unwrap();

            let sample_rate = track
                .codec_params
                .sample_rate
                .ok_or(anyhow::anyhow!("Unable to determine sample rate"))
                .unwrap() as usize;

            let mut format = probed.format;

            let track = format
                .tracks()
                .iter()
                .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                .unwrap();

            let dec_opts = DecoderOptions::default();
            let mut decoder = symphonia::default::get_codecs()
                .make(&track.codec_params, &dec_opts)
                .unwrap();

            let mut audio_buffer: VecDeque<f32> = VecDeque::new();

            let track_id = track.id;
            while let Ok(packet) = format.next_packet() {
                while !format.metadata().is_latest() {
                    format.metadata().pop();
                }
                if packet.track_id() != track_id {
                    continue;
                }
                let decoded = decoder.decode(&packet).unwrap();
                let mut sample_buf =
                    SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
                sample_buf.copy_interleaved_ref(decoded);
                let samples: &[f32] = bytemuck::cast_slice(sample_buf.samples());

                {
                    let buffer = &mut audio_buffer;
                    for &sample in samples {
                        buffer.push_back(sample);
                    }
                }
            }

            let params = OutputDeviceParameters {
                channels_count,
                sample_rate,
                channel_sample_count: audio_buffer.len(),
            };

            let _device = run_output_device(params, {
                move |data| {
                    let buffer = &mut audio_buffer;
                    data.iter_mut().for_each(|sample| {
                        if let Some(audio_sample) = buffer.pop_front() {
                            *sample = audio_sample;
                        } else {
                            *sample = 0.0;
                        }
                    });
                }
            })
            .unwrap();

            std::thread::sleep(std::time::Duration::from_secs(1));
        });

        Ok(())
    }

    pub fn mute(&mut self) {
        self.muted = true;
    }

    pub fn unmute(&mut self) {
        self.muted = false;
    }

    pub fn muted(&self) -> bool {
        self.muted
    }
}
