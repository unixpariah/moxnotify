use std::{fs, io::Read, path::Path, sync::Arc};

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
}

impl Audio {
    pub fn new() -> anyhow::Result<Self> {
        let mut mainloop = Mainloop::new().ok_or(PAErr(0))?;
        let mut context = Context::new(&mainloop, "audio-playback").ok_or(PAErr(0))?;
        mainloop.start()?;
        context.connect(None, context::FlagSet::NOFLAGS, None)?;

        loop {
            match context.get_state() {
                State::Ready => break,
                State::Failed | State::Terminated => {
                    return Err(anyhow::anyhow!("Failed to connect context"))
                }
                _ => {}
            }
        }

        Ok(Self { mainloop, context })
    }

    pub fn play(&mut self, audio_file: &Path) -> anyhow::Result<()> {
        let spec = Spec {
            format: libpulse_binding::sample::Format::S16le,
            channels: 2,
            rate: 44100,
        };

        if !spec.is_valid() {
            return Err(anyhow::anyhow!("Invalid PulseAudio specification"));
        }

        let mut stream = Stream::new(&mut self.context, "audio-playback", &spec, None)
            .ok_or(anyhow::anyhow!("Failed to create stream"))?;

        let buf_size = u32::pow(2, 15);
        let buf_attr = BufferAttr {
            maxlength: buf_size * 4,
            tlength: buf_size * 4,
            prebuf: buf_size,
            minreq: buf_size,
            fragsize: buf_size,
        };

        stream.connect_playback(
            None,
            Some(&buf_attr),
            stream::FlagSet::INTERPOLATE_TIMING
                | stream::FlagSet::AUTO_TIMING_UPDATE
                | stream::FlagSet::ADJUST_LATENCY
                | stream::FlagSet::START_CORKED,
            None,
            None,
        )?;

        loop {
            match stream.get_state() {
                stream::State::Ready => break,
                stream::State::Failed | stream::State::Terminated => {
                    self.mainloop.stop();
                    return Err(anyhow::anyhow!("Stream connection failed"));
                }
                _ => self.mainloop.wait(),
            }
        }

        stream.uncork(None);

        let mut file = fs::File::open(audio_file)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        let mut offset = 0;
        while offset < bytes.len() {
            let buffer = match stream.begin_write(None) {
                Ok(Some(buf)) => buf,
                Ok(None) => {
                    continue;
                }
                Err(e) => return Err(anyhow::anyhow!("Write error: {:?}", e)),
            };

            let remaining = bytes.len() - offset;
            let write_size = std::cmp::min(buffer.len(), remaining);

            buffer[..write_size].copy_from_slice(&bytes[offset..offset + write_size]);
            offset += write_size;

            stream.write(buffer, None, 0, stream::SeekMode::Relative)?;
        }

        let drained = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let drained = Arc::clone(&drained);
            stream.drain(Some(Box::new(move |_| {
                drained.store(true, std::sync::atomic::Ordering::SeqCst);
            })));
        }

        while !drained.load(std::sync::atomic::Ordering::SeqCst) {}

        Ok(())
    }
}
