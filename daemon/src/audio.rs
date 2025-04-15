use libpulse_binding::{
    context::{self, Context, State},
    error::PAErr,
    mainloop::threaded::Mainloop,
    sample::Spec,
    stream::{self, Stream},
};
use std::{cell::RefCell, fs, path::Path, rc::Rc, sync::Arc, thread};
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
    muted: bool,
    mainloop: Mainloop,
    context: Context,
}

impl Audio {
    pub fn new() -> anyhow::Result<Self> {
        let mut mainloop = Mainloop::new().ok_or(PAErr(0))?;
        let mut context = Context::new(&mainloop, "moxnotify").ok_or(PAErr(0))?;
        context.connect(None, context::FlagSet::NOFLAGS, None)?;
        mainloop.start()?;

        while context.get_state() != State::Ready {
            mainloop.wait();
        }

        Ok(Self {
            muted: false,
            mainloop,
            context,
        })
    }

    pub fn play(&mut self, path: Arc<Path>) -> anyhow::Result<()> {
        if self.muted {
            return Ok(());
        }

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
            .unwrap();

        let mut format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(anyhow::anyhow!(""))?;

        let mut stream = Stream::new(
            &mut self.context,
            "moxnotify",
            &Spec {
                format: libpulse_binding::sample::Format::FLOAT32NE,
                channels: track
                    .codec_params
                    .channels
                    .map(|channels| channels.count())
                    .unwrap_or(1) as u8,
                rate: track.codec_params.sample_rate.unwrap_or(44100),
            },
            None,
        )
        .ok_or(PAErr(0))?;

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
            self.mainloop.wait();
        }

        thread::spawn(move || {
            let stream = Rc::new(RefCell::new(stream));

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

                let writable = stream.borrow().writable_size();
                let samples: &[u8] = bytemuck::cast_slice(sample_buf.samples());

                stream
                    .borrow_mut()
                    .write(
                        if samples.len() > writable.unwrap_or_default() {
                            &samples[..writable.unwrap_or_default()]
                        } else {
                            samples
                        },
                        None,
                        0,
                        libpulse_binding::stream::SeekMode::Relative,
                    )
                    .unwrap();
            }

            stream.borrow_mut().drain(Some(Box::new({
                let stream = Rc::clone(&stream);
                move |_: bool| {
                    stream.borrow_mut().cork(None);
                }
            })));
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
