use std::{fs, path::Path, thread};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    formats::{FormatOptions, FormatReader},
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
            _ = run_output_device(params, {
                let mut clock = 0f32;
                move |data| {
                    for samples in data.chunks_mut(params.channels_count) {
                        clock = (clock + 1.0) % params.sample_rate as f32;
                        let value = (clock * 440.0 * 2.0 * std::f32::consts::PI
                            / params.sample_rate as f32)
                            .sin();
                        for sample in samples {
                            *sample = value;
                        }
                    }
                }
            })
            .unwrap();

            std::thread::sleep(std::time::Duration::from_secs(5));
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
