use crate::{
    config::{Key, KeyAction, KeyCombination, Modifiers},
    Moxnotify,
};
use ::xkbcommon::xkb::Context;
use calloop::{
    timer::{TimeoutAction, Timer},
    RegistrationToken,
};
use std::time::Duration;
use wayland_client::{
    protocol::{wl_keyboard, wl_seat},
    Connection, Dispatch, QueueHandle, WEnum,
};
use xkbcommon::xkb::{Keymap, State};

struct Xkb {
    context: Context,
    state: Option<State>,
}

pub struct Keyboard {
    _wl_keyboard: wl_keyboard::WlKeyboard,
    repeat: RepeatInfo,
    xkb: Xkb,
    pub key_combination: KeyCombination,
}

#[derive(Default)]
struct RepeatInfo {
    key: Option<Key>,
    rate: i32,
    delay: i32,
    registration_token: Option<RegistrationToken>,
}

impl Keyboard {
    pub fn new(qh: &QueueHandle<Moxnotify>, wl_seat: &wl_seat::WlSeat) -> Self {
        let wl_keyboard = wl_seat.get_keyboard(qh, ());

        let xkb_context = Context::new(0);

        Self {
            key_combination: KeyCombination::default(),
            xkb: Xkb {
                context: xkb_context,
                state: None,
            },
            _wl_keyboard: wl_keyboard,
            repeat: RepeatInfo::default(),
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: <wl_keyboard::WlKeyboard as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                let keymap_result = unsafe {
                    Keymap::new_from_fd(
                        &state.seat.keyboard.xkb.context,
                        fd,
                        size as usize,
                        format.into(),
                        0,
                    )
                };

                match keymap_result {
                    Ok(Some(keymap)) => {
                        let xkb_state = State::new(&keymap);
                        state.seat.keyboard.xkb.state = Some(xkb_state);
                    }
                    Ok(None) => {
                        log::error!("Keymap data was unexpectedly empty.");
                    }
                    Err(err) => {
                        log::error!("Failed to create keymap: {}", err);
                    }
                }
            }
            wl_keyboard::Event::Modifiers {
                serial: _,
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
            } => {
                if let Some(xkb_state) = state.seat.keyboard.xkb.state.as_mut() {
                    xkb_state.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);

                    state.seat.keyboard.key_combination.modifiers = Modifiers {
                        control: xkb_state
                            .mod_name_is_active("Control", xkbcommon::xkb::STATE_MODS_EFFECTIVE),
                        shift: xkb_state
                            .mod_name_is_active("Shift", xkbcommon::xkb::STATE_MODS_EFFECTIVE),
                        alt: xkb_state
                            .mod_name_is_active("Mod1", xkbcommon::xkb::STATE_MODS_EFFECTIVE),
                        meta: xkb_state
                            .mod_name_is_active("Mod4", xkbcommon::xkb::STATE_MODS_EFFECTIVE),
                    };
                }
            }
            wl_keyboard::Event::Key {
                serial: _,
                time: _,
                key,
                state: WEnum::Value(value),
            } => {
                // The wayland protocol gives us an input event code. To convert this to an xkb
                // keycode we must add 8.
                let keycode = key + 8;

                match value {
                    wl_keyboard::KeyState::Released => {
                        state.seat.keyboard.repeat.key = None;
                        if let Some(xkb_state) = state.seat.keyboard.xkb.state.as_ref() {
                            if Some(&vec![Key::from_keycode(xkb_state, keycode.into())])
                                != Some(&state.seat.keyboard.key_combination.keys)
                            {
                                return;
                            }
                        }

                        if let Some(token) = state.seat.keyboard.repeat.registration_token.take() {
                            state.loop_handle.remove(token);
                        }
                    }
                    wl_keyboard::KeyState::Pressed => {
                        if let Some(xkb_state) = state.seat.keyboard.xkb.state.as_ref() {
                            let key = Key::from_keycode(xkb_state, keycode.into());
                            state.seat.keyboard.repeat.key = Some(key);
                            state.seat.keyboard.key_combination.keys.push(key);

                            if xkb_state.get_keymap().key_repeats(keycode.into()) {
                                if let Some(token) =
                                    state.seat.keyboard.repeat.registration_token.take()
                                {
                                    state.loop_handle.remove(token);
                                }

                                let timer = Timer::from_duration(Duration::from_millis(
                                    state.seat.keyboard.repeat.delay as u64,
                                ));
                                let rate = (1000 / state.seat.keyboard.repeat.rate) as u64;
                                state.seat.keyboard.repeat.registration_token = state
                                    .loop_handle
                                    .insert_source(timer, move |_, _, moxnotify| {
                                        if let Some(key) = moxnotify.seat.keyboard.repeat.key {
                                            moxnotify.seat.keyboard.key_combination.keys.push(key);
                                        }
                                        if moxnotify.handle_key().is_err() {
                                            return TimeoutAction::Drop;
                                        }
                                        TimeoutAction::ToDuration(Duration::from_millis(rate))
                                    })
                                    .ok();
                            } else if let Some(token) =
                                state.seat.keyboard.repeat.registration_token
                            {
                                state.loop_handle.remove(token);
                            }
                        }

                        _ = state.handle_key();
                    }
                    _ => unreachable!(),
                }
            }
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                state.seat.keyboard.repeat.delay = delay;
                state.seat.keyboard.repeat.rate = rate;
            }
            _ => {}
        }
    }
}

impl Moxnotify {
    fn handle_key(&mut self) -> anyhow::Result<()> {
        if !self
            .config
            .keymaps
            .matches(&self.seat.keyboard.key_combination.keys)
        {
            self.seat.keyboard.key_combination.keys.drain(
                ..self
                    .seat
                    .keyboard
                    .key_combination
                    .keys
                    .len()
                    .saturating_sub(1),
            );
        }

        if let Some(action) = self.config.keymaps.get(&self.seat.keyboard.key_combination) {
            match action {
                KeyAction::Noop => return Ok(()),
                KeyAction::NextNotification => self.notifications.next(),
                KeyAction::PreviousNotification => self.notifications.prev(),
                KeyAction::FirstNotification => {
                    while self.notifications.selected()
                        != self.notifications.first().map(|n| n.id())
                    {
                        self.notifications.prev();
                    }
                }
                KeyAction::LastNotification => {
                    while self.notifications.selected() != self.notifications.last().map(|n| n.id())
                    {
                        self.notifications.next();
                    }
                }
                KeyAction::DismissNotification => {
                    if let Some(id) = self.notifications.selected() {
                        if let Some(index) = self
                            .notifications
                            .iter()
                            .position(|notification| notification.id() == id)
                        {
                            self.notifications.dismiss(id);
                            let adjusted_index = if index == self.notifications.len() {
                                index.saturating_sub(1)
                            } else {
                                index
                            };

                            if let Some(notification) =
                                self.notifications.get(adjusted_index).map(|n| n.id())
                            {
                                self.notifications.select(notification);
                            }
                        }
                    }
                }
                KeyAction::Unfocus => {
                    if let Some(surface) = self.surface.as_mut() {
                        surface.unfocus();
                        self.seat.keyboard.key_combination.keys.clear();
                        self.notifications.deselect();
                    }
                }
            }
        } else {
            return Err(anyhow::anyhow!(""));
        }

        self.update_surface_size();
        if let Some(surface) = self.surface.as_mut() {
            _ = surface.render(
                &self.wgpu_state.device,
                &self.wgpu_state.queue,
                &self.notifications,
            );
        }

        self.seat.keyboard.key_combination.clear();

        Ok(())
    }
}
