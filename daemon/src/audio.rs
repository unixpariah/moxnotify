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

pub struct Audio {
    _mainloop: Mainloop,
    _context: Context,
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
            format: libpulse_binding::sample::Format::S16le,
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
            _mainloop: mainloop,
            _context: context,
        })
    }

    pub fn play(&mut self, path: Arc<Path>) -> anyhow::Result<()> {
        let stream = self.stream.clone();

        thread::spawn(move || -> anyhow::Result<()> {
            let src = fs::File::open(path)?;
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

                let i16_samples = sample_buf
                    .samples()
                    .iter()
                    .map(|sample| (sample.clamp(-1.0, 1.0) * 32767.0) as i16)
                    .collect::<Vec<_>>();

                _ = stream.lock().unwrap().write(
                    bytemuck::cast_slice(&i16_samples),
                    None,
                    0,
                    libpulse_binding::stream::SeekMode::Relative,
                );
            }
            Ok(())
        });

        Ok(())
    }
}
