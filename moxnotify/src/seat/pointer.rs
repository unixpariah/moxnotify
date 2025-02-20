use crate::Moxnotify;
use wayland_client::{
    globals::GlobalList,
    protocol::{wl_compositor, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_cursor::CursorTheme;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::KeyboardInteractivity;

#[derive(PartialEq, Debug)]
enum PointerState {
    Pressed,
    Grabbing,
    Default,
    Hover,
}

pub struct Pointer {
    state: PointerState,
    x: f64,
    y: f64,
    wl_pointer: wl_pointer::WlPointer,
    surface: wl_surface::WlSurface,
    theme: CursorTheme,
    scroll_accumulator: f64,
}

impl Pointer {
    pub fn new(
        conn: &Connection,
        qh: &QueueHandle<Moxnotify>,
        compositor: &wl_compositor::WlCompositor,
        globals: &GlobalList,
        wl_seat: &wl_seat::WlSeat,
    ) -> anyhow::Result<Self> {
        let wl_pointer = wl_seat.get_pointer(qh, ());
        let surface = compositor.create_surface(qh, ());
        let shm = globals.bind::<wl_shm::WlShm, _, _>(qh, 1..=2, ())?;

        let cursor_theme = CursorTheme::load(conn, shm, 32)?;

        Ok(Self {
            state: PointerState::Default,
            x: 0.,
            y: 0.,
            theme: cursor_theme,
            wl_pointer,
            surface,
            scroll_accumulator: 0.,
        })
    }

    fn change_state(&mut self, pointer_state: PointerState) {
        if self.state == pointer_state {
            return;
        }

        match pointer_state {
            PointerState::Default => {
                if let Some(buffer) = self.theme.get_cursor("default").as_ref() {
                    self.surface.attach(Some(&buffer[0]), 0, 0);
                    self.surface.damage_buffer(0, 0, 32, 32);
                    self.surface.commit();
                }
            }
            PointerState::Pressed => {}
            PointerState::Grabbing => {
                if let Some(buffer) = self.theme.get_cursor("grabbing").as_ref() {
                    self.surface.attach(Some(&buffer[0]), 0, 0);
                    self.surface.damage_buffer(0, 0, 32, 32);
                    self.surface.commit();
                }
            }
            PointerState::Hover => {
                if let Some(buffer) = self.theme.get_cursor("pointer").as_ref() {
                    self.surface.attach(Some(&buffer[0]), 0, 0);
                    self.surface.damage_buffer(0, 0, 32, 32);
                    self.surface.commit();
                }
            }
        }

        self.state = pointer_state;
    }
}

const LEFT_MOUSE_CLICK: u32 = 272;

impl Dispatch<wl_pointer::WlPointer, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        _proxy: &wl_pointer::WlPointer,
        event: <wl_pointer::WlPointer as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Motion {
                time: _,
                surface_x,
                surface_y,
            } => {
                let hovered_id = state
                    .notifications
                    .get_by_coordinates(surface_x, surface_y)
                    .map(|n| n.id());

                let pointer = &mut state.seat.pointer;
                pointer.x = surface_x;
                pointer.y = surface_y;

                if let PointerState::Grabbing = pointer.state {
                    return;
                }

                if let PointerState::Pressed = pointer.state {
                    if state.notifications.selected().is_some() {
                        pointer.change_state(PointerState::Grabbing);
                    }
                    return;
                }

                match (hovered_id, state.notifications.selected()) {
                    (Some(new_id), Some(old_id)) if new_id != old_id => {
                        state.select_notification(new_id);
                        state.seat.pointer.change_state(PointerState::Hover);
                    }
                    (Some(new_id), None) => {
                        state.select_notification(new_id);
                        state.seat.pointer.change_state(PointerState::Hover);
                    }
                    (None, Some(_)) => {
                        state.deselect_notification();
                        state.seat.pointer.change_state(PointerState::Default);
                    }
                    _ => {}
                }
            }
            wl_pointer::Event::Button {
                serial,
                time: _,
                button,
                state: WEnum::Value(value),
            } => {
                if button != LEFT_MOUSE_CLICK {
                    return;
                }

                match value {
                    wl_pointer::ButtonState::Pressed => {
                        state.seat.pointer.state = PointerState::Pressed;
                    }
                    wl_pointer::ButtonState::Released => {
                        if let Some(id) = state.notifications.selected() {
                            let pointer = &state.seat.pointer;
                            if state.notifications.get_by_id(id)
                                == state.notifications.get_by_coordinates(pointer.x, pointer.y)
                            {
                                state.invoke_action(id, serial);
                                if let Some(notification) = state
                                    .notifications
                                    .get_by_coordinates(state.seat.pointer.x, state.seat.pointer.y)
                                {
                                    state.select_notification(notification.id());
                                    state.seat.pointer.change_state(PointerState::Hover);
                                    return;
                                }
                            } else {
                                let pointer = &state.seat.pointer;
                                if let Some(notification) =
                                    state.notifications.get_by_coordinates(pointer.x, pointer.y)
                                {
                                    state.select_notification(notification.id());
                                    state.seat.pointer.change_state(PointerState::Hover);
                                    return;
                                }
                            }
                        }

                        state.seat.pointer.change_state(PointerState::Default);
                    }
                    _ => unreachable!(),
                }
            }
            wl_pointer::Event::Leave {
                serial: _,
                surface: _,
            } => {
                if let Some(layer_surface) = state.surface.layer_surface.as_ref() {
                    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                    state.surface.wl_surface.commit();
                }
                state.deselect_notification();
            }
            wl_pointer::Event::Enter {
                serial,
                surface: _,
                surface_x,
                surface_y,
            } => {
                if let Some(layer_surface) = state.surface.layer_surface.as_ref() {
                    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
                    state.surface.wl_surface.commit();
                }

                state.seat.pointer.x = surface_x;
                state.seat.pointer.y = surface_y;

                let hovered_id = state
                    .notifications
                    .get_by_coordinates(surface_x, surface_y)
                    .map(|n| n.id());

                match hovered_id {
                    Some(id) => {
                        state.select_notification(id);
                        let pointer = &mut state.seat.pointer;
                        if let Some(buffer) = pointer.theme.get_cursor("hover").as_ref() {
                            pointer.surface.attach(Some(&buffer[0]), 0, 0);
                            pointer.surface.damage_buffer(0, 0, 32, 32);
                            pointer.surface.commit();
                        }
                        state.seat.pointer.change_state(PointerState::Hover);
                    }
                    None => {
                        let pointer = &mut state.seat.pointer;
                        if let Some(buffer) = pointer.theme.get_cursor("default").as_ref() {
                            pointer.surface.attach(Some(&buffer[0]), 0, 0);
                            pointer.surface.damage_buffer(0, 0, 32, 32);
                            pointer.surface.commit();
                        }
                        state.seat.pointer.change_state(PointerState::Default);
                    }
                }

                state.seat.pointer.wl_pointer.set_cursor(
                    serial,
                    Some(&state.seat.pointer.surface),
                    0,
                    0,
                );
            }
            wl_pointer::Event::Axis {
                time: _,
                axis: WEnum::Value(axis),
                value,
            } => {
                if axis == wl_pointer::Axis::VerticalScroll {
                    state.seat.pointer.scroll_accumulator += value;

                    if state.seat.pointer.scroll_accumulator.abs()
                        >= state.config.scroll_sensitivity
                    {
                        if state.seat.pointer.scroll_accumulator.is_sign_positive() {
                            state.notifications.next();
                        } else {
                            state.notifications.prev();
                        }
                        state.render();

                        state.seat.pointer.scroll_accumulator = 0.0;
                    }
                }
            }
            _ => {}
        }
    }
}
