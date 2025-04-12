use crate::{
    button::ButtonType,
    config::keymaps::{Key, KeyAction, KeyWithModifiers, Keys, Mode, Modifiers},
    notification_manager::Reason,
    EmitEvent, History, Moxnotify,
};
use calloop::{
    timer::{TimeoutAction, Timer},
    RegistrationToken,
};
use std::{sync::Arc, time::Duration};
use wayland_client::{
    protocol::{wl_keyboard, wl_seat},
    Connection, Dispatch, QueueHandle, WEnum,
};
use xkbcommon::xkb::{Context, Keymap, State};

struct Xkb {
    context: Context,
    state: Option<State>,
}

pub struct Keyboard {
    _wl_keyboard: wl_keyboard::WlKeyboard,
    pub repeat: RepeatInfo,
    xkb: Xkb,
    pub key_combination: Keys,
    modifiers: Modifiers,
    pub mode: Mode,
}

#[derive(Default)]
pub struct RepeatInfo {
    pub key: Option<Key>,
    rate: i32,
    delay: i32,
    registration_token: Option<RegistrationToken>,
}

impl Keyboard {
    pub fn new(qh: &QueueHandle<Moxnotify>, wl_seat: &wl_seat::WlSeat) -> Self {
        let wl_keyboard = wl_seat.get_keyboard(qh, ());

        let xkb_context = Context::new(0);

        Self {
            mode: Mode::Normal,
            key_combination: Keys(Vec::new()),
            xkb: Xkb {
                context: xkb_context,
                state: None,
            },
            _wl_keyboard: wl_keyboard,
            repeat: RepeatInfo::default(),
            modifiers: Modifiers::default(),
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
                }
                .ok()
                .flatten();

                match keymap_result {
                    Some(keymap) => {
                        let xkb_state = State::new(&keymap);
                        state.seat.keyboard.xkb.state = Some(xkb_state);
                    }
                    None => {
                        log::error!("Keymap data was unexpectedly empty.");
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

                    state.seat.keyboard.modifiers = Modifiers {
                        control: xkb_state
                            .mod_name_is_active("Control", xkbcommon::xkb::STATE_MODS_EFFECTIVE),
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
                            if let Some(key) = Key::from_keycode(xkb_state, keycode.into()) {
                                let key_with_modifiers = KeyWithModifiers {
                                    key,
                                    modifiers: state.seat.keyboard.modifiers,
                                };

                                if Keys(vec![key_with_modifiers])
                                    != state.seat.keyboard.key_combination
                                {
                                    return;
                                }
                            }
                        }

                        if let Some(token) = state.seat.keyboard.repeat.registration_token.take() {
                            state.loop_handle.remove(token);
                        }
                    }
                    wl_keyboard::KeyState::Pressed => {
                        if let Some(xkb_state) = state.seat.keyboard.xkb.state.as_ref() {
                            let key = Key::from_keycode(xkb_state, keycode.into());
                            state.seat.keyboard.repeat.key = key;
                            if let Some(key) = key {
                                let key_with_modifiers = KeyWithModifiers {
                                    key,
                                    modifiers: state.seat.keyboard.modifiers,
                                };
                                state.seat.keyboard.key_combination.push(key_with_modifiers);
                            }

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
                                            let key_with_modifiers = KeyWithModifiers {
                                                key,
                                                modifiers: moxnotify.seat.keyboard.modifiers,
                                            };
                                            moxnotify
                                                .seat
                                                .keyboard
                                                .key_combination
                                                .push(key_with_modifiers);
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
            .matches(&self.seat.keyboard.key_combination)
        {
            let len = self.seat.keyboard.key_combination.len().saturating_sub(1);
            self.seat.keyboard.key_combination.drain(..len);
        }

        if let Some(key_combination) = self.config.keymaps.iter().find(|keymap| {
            keymap.keys == self.seat.keyboard.key_combination
                && keymap.mode == self.seat.keyboard.mode
        }) {
            match key_combination.action {
                KeyAction::Noop => {}
                KeyAction::NextNotification => self.notifications.next(),
                KeyAction::PreviousNotification => self.notifications.prev(),
                KeyAction::FirstNotification => {
                    while self.notifications.selected_id()
                        != self.notifications.first().map(|n| n.id())
                    {
                        self.notifications.prev();
                    }
                }
                KeyAction::LastNotification => {
                    while self.notifications.selected_id()
                        != self.notifications.last().map(|n| n.id())
                    {
                        self.notifications.next();
                    }
                }
                KeyAction::DismissNotification => {
                    if let Some(id) = self.notifications.selected_id() {
                        self.dismiss_by_id(id, Some(Reason::DismissedByUser));
                    }
                }
                KeyAction::Unfocus => {
                    if let Some(surface) = self.surface.as_mut() {
                        surface.unfocus();
                        self.seat.keyboard.key_combination.clear();
                        self.notifications.deselect();
                        self.seat.keyboard.repeat.key = None;
                    }
                }
                KeyAction::HintMode => self.seat.keyboard.mode = Mode::Hint,
                KeyAction::ShowHistory => self.handle_app_event(crate::Event::ShowHistory)?,
                KeyAction::HideHistory => self.handle_app_event(crate::Event::HideHistory)?,
                KeyAction::ToggleHistory => {
                    match self.history {
                        History::Shown => self.handle_app_event(crate::Event::HideHistory),
                        History::Hidden => self.handle_app_event(crate::Event::ShowHistory),
                    }?;
                }
                KeyAction::Uninhibit => self.inhibited = false,
                KeyAction::Ihibit => self.inhibited = true,
                KeyAction::ToggleInhibit => self.inhibited = !self.inhibited,
                KeyAction::Mute => {
                    if let Some(audio) = self.audio.as_mut() {
                        audio.mute();
                    }
                }
                KeyAction::Unmute => {
                    if let Some(audio) = self.audio.as_mut() {
                        audio.unmute();
                    }
                }
                KeyAction::ToggleMute => {
                    if let Some(audio) = self.audio.as_mut() {
                        match audio.muted() {
                            true => audio.unmute(),
                            false => audio.mute(),
                        }
                    }
                }
                KeyAction::NormalMode => self.seat.keyboard.mode = Mode::Normal,
            }
        } else {
            let combination = self.seat.keyboard.key_combination.to_string();
            if let Some(notification) = self.notifications.selected_notification_mut() {
                let id = notification.id();

                match notification.buttons.get_by_character(&combination) {
                    Some(ButtonType::Dismiss) => {
                        self.dismiss_by_id(id, Some(Reason::DismissedByUser))
                    }
                    Some(ButtonType::Action { action, .. }) => {
                        if let Some(surface) = self.surface.as_ref() {
                            let token = surface.token.as_ref().map(Arc::clone);
                            _ = self.emit_sender.send(EmitEvent::ActionInvoked {
                                id,
                                action_key: action,
                                token: token.unwrap_or_default(),
                            });
                        }

                        if !notification.data.hints.resident {
                            self.dismiss_by_id(id, Some(Reason::DismissedByUser));
                        } else {
                            self.seat.keyboard.mode = Mode::Normal;
                        }
                    }
                    Some(ButtonType::Anchor { anchor }) => {
                        if let Some(surface) = self.surface.as_ref() {
                            let token = surface.token.as_ref().map(Arc::clone);
                            if self
                                .emit_sender
                                .send(EmitEvent::Open {
                                    uri: Arc::clone(&anchor.href),
                                    token,
                                })
                                .is_ok()
                            {
                                self.notifications.deselect();
                                self.seat.keyboard.mode = Mode::Normal;
                            }
                        }
                    }
                    None => {}
                }
            }
        }

        self.update_surface_size();
        if let Some(surface) = self.surface.as_mut() {
            _ = surface.render(
                self.seat.keyboard.mode,
                &self.wgpu_state.device,
                &self.wgpu_state.queue,
                &self.notifications,
            );
        }

        self.seat.keyboard.key_combination.clear();

        Ok(())
    }
}
