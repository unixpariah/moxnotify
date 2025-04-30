use libpulse_binding::{
    context::{self, Context, State},
    error::PAErr,
    mainloop::threaded::Mainloop,
    sample::Spec,
    stream::{self, Stream},
};
use std::{
    cell::RefCell,
    fs,
    path::Path,
    rc::Rc,
    sync::{mpsc, Arc, Mutex},
    thread::{self, JoinHandle},
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    formats::{FormatOptions, FormatReader},
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

struct Playback {
    mainloop: Rc<RefCell<Mainloop>>,
    stream: Arc<Mutex<Stream>>,
    shutdown_channel: (mpsc::Sender<()>, Option<mpsc::Receiver<()>>),
    handle: Option<JoinHandle<()>>,
    format: Option<Box<dyn FormatReader>>,
}

impl Playback {
    fn new<T>(
        path: T,
        context: Arc<Mutex<Context>>,
        mainloop: Rc<RefCell<Mainloop>>,
        stream: Option<Arc<Mutex<Stream>>>,
    ) -> anyhow::Result<Self>
    where
        T: AsRef<Path>,
    {
        let shutdown_channel = mpsc::channel();

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

        if let Some(stream) = stream {
            return Ok(Self {
                mainloop,
                stream,
                shutdown_channel: (shutdown_channel.0, Some(shutdown_channel.1)),
                format: Some(probed.format),
                handle: None,
            });
        }

        let track = probed
            .format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(anyhow::anyhow!("No valid audio track found"))?;

        let channels = track
            .codec_params
            .channels
            .map(|channels| channels.count())
            .ok_or(anyhow::anyhow!("Unable to determine channel count"))?;

        let sample_rate = track
            .codec_params
            .sample_rate
            .ok_or(anyhow::anyhow!("Unable to determine sample rate"))?;

        let mut context = context.lock().unwrap();

        let mut stream = Stream::new(
            &mut context,
            "moxnotify",
            &Spec {
                format: libpulse_binding::sample::Format::FLOAT32NE,
                channels: channels as u8,
                rate: sample_rate,
            },
            None,
        )
        .ok_or(anyhow::anyhow!("Failed to create audio stream"))?;

        mainloop.borrow_mut().lock();
        stream.connect_playback(
            None,
            None,
            stream::FlagSet::INTERPOLATE_TIMING
                | stream::FlagSet::AUTO_TIMING_UPDATE
                | stream::FlagSet::EARLY_REQUESTS,
            None,
            None,
        )?;
        mainloop.borrow_mut().unlock();

        while stream.get_state() != stream::State::Ready {
            mainloop.borrow_mut().wait();
        }

        Ok(Self {
            mainloop,
            stream: Arc::new(Mutex::new(stream)),
            shutdown_channel: (shutdown_channel.0, Some(shutdown_channel.1)),
            format: Some(probed.format),
            handle: None,
        })
    }

    fn play(&mut self) -> anyhow::Result<()> {
        if self.format.is_none() || self.shutdown_channel.1.is_none() {
            return Err(anyhow::anyhow!("Playback already played"));
        }

        let mut format = self.format.take().unwrap();
        let rx = self.shutdown_channel.1.take().unwrap();

        let stream = Arc::clone(&self.stream);
        let handle = thread::spawn(move || {
            let stream = stream;

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
                if rx.try_recv().is_ok() {
                    break;
                }

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

                let writable = stream.lock().unwrap().writable_size();
                let samples: &[u8] = bytemuck::cast_slice(sample_buf.samples());

                stream
                    .lock()
                    .unwrap()
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
        });

        self.handle = Some(handle);

        Ok(())
    }

    fn stop(&mut self) -> Arc<Mutex<Stream>> {
        _ = self.shutdown_channel.0.send(());
        if let Some(handle) = self.handle.take() {
            _ = handle.join();
        }

        self.mainloop.borrow_mut().lock();
        self.stream.lock().unwrap().flush(None);
        self.mainloop.borrow_mut().unlock();

        Arc::clone(&self.stream)
    }
}

#[allow(dead_code)]
pub struct Audio {
    muted: bool,
    mainloop: Rc<RefCell<Mainloop>>,
    context: Arc<Mutex<Context>>,
    stream: Option<Arc<Mutex<Stream>>>,
    playback: Option<Playback>,
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
            stream: None,
            muted: false,
            mainloop: Rc::new(RefCell::new(mainloop)),
            context: Arc::new(Mutex::new(context)),
            playback: None,
        })
    }

    pub fn play<T>(&mut self, path: T) -> anyhow::Result<()>
    where
        T: AsRef<Path>,
    {
        if self.muted {
            return Ok(());
        }

        if let Some(mut playback) = self.playback.take() {
            self.stream = Some(playback.stop());
            while self.context.lock().unwrap().get_state() != State::Ready {
                self.mainloop.borrow_mut().wait();
            }
        }

        let mut playback = Playback::new(
            path,
            self.context.clone(),
            Rc::clone(&self.mainloop),
            self.stream.as_ref().map(Arc::clone),
        )?;
        while self.context.lock().unwrap().get_state() != State::Ready {
            self.mainloop.borrow_mut().wait();
        }
        playback.play()?;

        self.playback = Some(playback);

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
