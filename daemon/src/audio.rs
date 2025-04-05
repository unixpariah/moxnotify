use libpulse_binding::{
    context::{self, Context, State},
    def::BufferAttr,
    error::PAErr,
    mainloop::threaded::Mainloop,
    sample::Spec,
    stream::{self, Stream},
};
use rand::Rng;
use std::{cell::RefCell, rc::Rc};

pub struct Audio {
    mainloop: Mainloop,
    context: Context,
    stream: Rc<RefCell<Stream>>,
    spec: Spec,
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

        let stream = Rc::new(RefCell::new(
            Stream::new(&mut context, "audio-playback", &spec, None).ok_or(PAErr(0))?,
        ));

        Self::connect_playback(stream.clone(), &spec);

        Ok(Self {
            stream,
            mainloop,
            context,
            spec,
        })
    }

    fn connect_playback(stream: Rc<RefCell<Stream>>, spec: &Spec) {
        let mut fragment_size = 0;
        let mut n_fragments = 0;

        stream.borrow_mut().set_write_callback(Some(Box::new({
            let stream = stream.clone();
            move |size| {
                let mut stream = stream.borrow_mut();
                Self::write_cb(&mut stream, size);
            }
        })));

        stream
            .borrow_mut()
            .set_overflow_callback(Some(Box::new(Self::stream_over_cb)));
        stream
            .borrow_mut()
            .set_underflow_callback(Some(Box::new(Self::stream_under_cb)));

        let fs = spec.frame_size();

        if n_fragments < 2 {
            if fragment_size > 0 {
                n_fragments = spec.bytes_per_second() / 2 / fragment_size;
                if n_fragments < 2 {
                    n_fragments = 2;
                }
            } else {
                n_fragments = 12;
            }
        }

        fragment_size = spec.bytes_per_second() / 2 / n_fragments;
        if fragment_size < 1024 {
            fragment_size = 1024;
        }

        fragment_size = (fragment_size / fs) * fs;
        if fragment_size == 0 {
            fragment_size = fs;
        }

        println!(
            "fragment_size: {}, n_fragments: {}, fs: {}",
            fragment_size, n_fragments, fs
        );

        let attr = BufferAttr {
            maxlength: (fragment_size * (n_fragments + 1)) as u32,
            tlength: (fragment_size * n_fragments) as u32,
            prebuf: fragment_size as u32,
            minreq: fragment_size as u32,
            ..Default::default()
        };

        let tmp = stream.borrow_mut().connect_playback(
            None,
            Some(&attr),
            stream::FlagSet::INTERPOLATE_TIMING
                | stream::FlagSet::AUTO_TIMING_UPDATE
                | stream::FlagSet::EARLY_REQUESTS,
            None,
            None,
        );

        if tmp.is_err() {
            println!("connect_playback returned {:?}", tmp);
        }
    }

    fn stream_over_cb() {
        log::warn!("Audio Device: stream overflow...");
    }

    fn stream_under_cb() {
        log::warn!("Audio Device: stream underflow...");
    }

    fn write_cb(stream: &mut Stream, size: usize) {
        let len = size / std::mem::size_of::<i16>();
        let mut data = vec![0i16; len];

        (0..len).for_each(|i| {
            let mut rng = rand::rng();
            data[i] = rng.random_range(-32768..=32767);
        });

        _ = stream.write(
            bytemuck::cast_slice(&data),
            None,
            0,
            libpulse_binding::stream::SeekMode::Relative,
        );
    }
}
