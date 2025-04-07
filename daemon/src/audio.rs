use libpulse_binding::{
    context::{self, Context, State},
    error::PAErr,
    mainloop::threaded::Mainloop,
    sample::Spec,
    stream::{self, Stream},
};
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    thread,
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

#[allow(dead_code)]
pub struct Audio {
    mainloop: Mainloop,
    context: Context,
    stream: Arc<Mutex<Stream>>,
}

impl Audio {
    pub fn new() -> anyhow::Result<Self> {
        let mut mainloop = Mainloop::new().ok_or(PAErr(0))?;
        let mut context = Context::new(&mainloop, "audio-playback").ok_or(PAErr(0))?;
        context.connect(None, context::FlagSet::NOFLAGS, None)?;
        mainloop.start()?;

        while context.get_state() != State::Ready {
            mainloop.wait();
        }

        let spec = Spec {
            format: libpulse_binding::sample::Format::FLOAT32NE,
            channels: 2,
            rate: 44100,
        };

        let mut stream =
            Stream::new(&mut context, "audio-playback", &spec, None).ok_or(PAErr(0))?;

        stream.connect_playback(
            None,
            None,
            stream::FlagSet::INTERPOLATE_TIMING
                | stream::FlagSet::AUTO_TIMING_UPDATE
                | stream::FlagSet::EARLY_REQUESTS,
            None,
            None,
        )?;

        while stream.get_state() != stream::State::Ready {
            mainloop.wait();
        }

        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
            mainloop,
            context,
        })
    }

    pub fn play(&mut self, path: Arc<Path>) -> anyhow::Result<()> {
        let stream = self.stream.clone();

        thread::spawn(move || {
            let src = match fs::File::open(Arc::clone(&path)) {
                Ok(file) => file,
                Err(_) => {
                    log::error!("Sound file {} doesn't exist", path.display());
                    return;
                }
            };

            let mss = MediaSourceStream::new(Box::new(src), Default::default());

            let hint = Hint::new();

            let probed = symphonia::default::get_probe()
                .format(
                    &hint,
                    mss,
                    &FormatOptions::default(),
                    &MetadataOptions::default(),
                )
                .unwrap();
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

            let track_id = track.id;

            let Ok(mut stream) = stream.lock() else {
                return;
            };
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

                _ = stream.write(
                    bytemuck::cast_slice(sample_buf.samples()),
                    None,
                    0,
                    libpulse_binding::stream::SeekMode::Relative,
                );
            }
        });

        Ok(())
    }
}
