use libpulse_binding::{
    context::{self, Context, State},
    def::BufferAttr,
    error::PAErr,
    mainloop::threaded::Mainloop,
    sample::Spec,
    stream::{self, Stream},
};

pub struct Audio {
    mainloop: Mainloop,
    context: Context,
    stream: Stream,
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
            channels: 1,
            rate: 44100,
        };

        let mut stream =
            Stream::new(&mut context, "audio-playback", &spec, None).ok_or(PAErr(0))?;

        stream.set_overflow_callback(Some(Box::new(Self::stream_over_cb)));
        stream.set_underflow_callback(Some(Box::new(Self::stream_under_cb)));

        let attr = Self::buffer_attr(&spec);

        stream.connect_playback(
            None,
            Some(&attr),
            stream::FlagSet::INTERPOLATE_TIMING
                | stream::FlagSet::AUTO_TIMING_UPDATE
                | stream::FlagSet::EARLY_REQUESTS,
            None,
            None,
        )?;

        Ok(Self {
            stream,
            mainloop,
            context,
            spec,
        })
    }

    pub fn play(&mut self) -> anyhow::Result<()> {
        let size = 35000;

        let len = size / std::mem::size_of::<i16>();
        let mut data = vec![0i16; len];

        let twopi_over_sr = std::f32::consts::PI * 2.0 / self.spec.rate as f32;

        (0..len).step_by(self.spec.channels as usize).for_each(|i| {
            let val = (32767.0 * 0.3 * (500. * i as f32 * twopi_over_sr).sin()) as i16;
            (0..self.spec.channels).for_each(|j| {
                data[i + j as usize] = val;
            });
        });

        _ = self.stream.write(
            bytemuck::cast_slice(&data),
            None,
            0,
            libpulse_binding::stream::SeekMode::Relative,
        );

        Ok(())
    }

    fn buffer_attr(spec: &Spec) -> BufferAttr {
        let mut fragment_size = 0;
        let mut n_fragments = 0;

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

        BufferAttr {
            maxlength: (fragment_size * (n_fragments + 1)) as u32,
            tlength: (fragment_size * n_fragments) as u32,
            prebuf: fragment_size as u32,
            minreq: fragment_size as u32,
            ..Default::default()
        }
    }

    fn stream_over_cb() {
        log::warn!("Audio Device: stream overflow...");
    }

    fn stream_under_cb() {
        log::warn!("Audio Device: stream underflow...");
    }
}
