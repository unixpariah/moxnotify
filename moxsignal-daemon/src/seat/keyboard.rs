use crate::{
    config::{Key, KeyAction, KeyCombination, Modifiers},
    Moxsignal,
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
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::KeyboardInteractivity;
use xkbcommon::xkb::{Keymap, State};

struct Xkb {
    context: Context,
    state: Option<State>,
}

pub struct Keyboard {
    wl_keyboard: wl_keyboard::WlKeyboard,
    repeat: RepeatInfo,
    xkb: Xkb,
    pub key_combination: KeyCombination,
}

#[derive(Default)]
struct RepeatInfo {
    rate: i32,
    delay: i32,
    registration_token: Option<RegistrationToken>,
}

impl Keyboard {
    pub fn new(qh: &QueueHandle<Moxsignal>, wl_seat: &wl_seat::WlSeat) -> Self {
        let wl_keyboard = wl_seat.get_keyboard(qh, ());

        let xkb_context = Context::new(0);

        Self {
            key_combination: KeyCombination::default(),
            xkb: Xkb {
                context: xkb_context,
                state: None,
            },
            wl_keyboard,
            repeat: RepeatInfo::default(),
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Moxsignal {
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
                serial,
                time: _,
                key,
                state: WEnum::Value(value),
            } => {
                // The wayland protocol gives us an input event code. To convert this to an xkb
                // keycode we must add 8.
                let keycode = key + 8;

                match value {
                    wl_keyboard::KeyState::Released => {
                        if let Some(xkb_state) = state.seat.keyboard.xkb.state.as_ref() {
                            if Some(Key::from_keycode(xkb_state, keycode.into()))
                                != Some(state.seat.keyboard.key_combination.key)
                            {
                                return;
                            }
                        }

                        state.seat.keyboard.key_combination.key = Key::Character('\0');
                        if let Some(token) = state.seat.keyboard.repeat.registration_token.take() {
                            state.loop_handle.remove(token);
                        }
                    }
                    wl_keyboard::KeyState::Pressed => {
                        if let Some(xkb_state) = state.seat.keyboard.xkb.state.as_ref() {
                            let key = Key::from_keycode(xkb_state, keycode.into());
                            state.seat.keyboard.key_combination.key = key;
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
                                    .insert_source(timer, move |_, _, moxsignal| {
                                        moxsignal.handle_key(serial);
                                        TimeoutAction::ToDuration(Duration::from_millis(rate))
                                    })
                                    .ok();
                            } else if let Some(token) =
                                state.seat.keyboard.repeat.registration_token
                            {
                                state.loop_handle.remove(token);
                            }
                        }

                        state.handle_key(serial);
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

impl Moxsignal {
    fn handle_key(&mut self, serial: u32) {
        if let Some(action) = self.config.keymaps.get(&self.seat.keyboard.key_combination) {
            match action {
                KeyAction::NextNotification => self.notifications.next(),
                KeyAction::PreviousNotification => self.notifications.prev(),
                KeyAction::InvokeAction => {
                    if let Some(id) = self.notifications.selected() {
                        if let Some(index) = self
                            .notifications
                            .iter()
                            .position(|notification| notification.id() == id)
                        {
                            self.invoke_action(id, serial);
                            let adjusted_index = if index == self.notifications.len()
                                && !self.notifications.is_empty()
                            {
                                index.saturating_sub(1)
                            } else {
                                index
                            };

                            if let Some(notification) =
                                self.notifications.get(adjusted_index).map(|n| n.id())
                            {
                                self.select_notification(notification);
                            }
                        }
                    }
                }
                KeyAction::DismissNotification => {
                    if let Some(id) = self.notifications.selected() {
                        if let Some(index) = self
                            .notifications
                            .iter()
                            .position(|notification| notification.id() == id)
                        {
                            self.dismiss_notification(id);
                            let adjusted_index = if index == self.notifications.len() {
                                index.saturating_sub(1)
                            } else {
                                index
                            };

                            if let Some(notification) =
                                self.notifications.get(adjusted_index).map(|n| n.id())
                            {
                                self.select_notification(notification);
                            }
                        }
                    }
                }
                KeyAction::Unfocus => {
                    if let Some(layer_surface) = self.surface.layer_surface.as_ref() {
                        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
                        self.surface.wl_surface.commit();
                        self.seat.keyboard.key_combination.key = Key::Character('\0');
                    }
                }
            }

            self.render();
        }
    }
}
