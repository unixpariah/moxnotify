use crate::{notification_manager::notification::button::Action, EmitEvent, Moxnotify};
use std::sync::Arc;
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
    state: Option<PointerState>,
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
            state: None,
            x: 0.,
            y: 0.,
            theme: cursor_theme,
            wl_pointer,
            surface,
            scroll_accumulator: 0.,
        })
    }

    fn change_state(&mut self, pointer_state: Option<PointerState>) {
        let Some(pointer_state) = pointer_state else {
            self.state = None;
            return;
        };

        if self.state.as_ref() == Some(&pointer_state) {
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

        self.state = Some(pointer_state);
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
        let Some(surface) = state.surface.as_mut() else {
            return;
        };

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

                if let Some(PointerState::Grabbing) = pointer.state {
                    return;
                }

                if let Some(PointerState::Pressed) = pointer.state {
                    if state.notifications.selected().is_some() {
                        pointer.change_state(Some(PointerState::Grabbing));
                    }
                    return;
                }

                let pointer = &state.seat.pointer;

                if state
                    .notifications
                    .get_button_by_coordinates(pointer.x, pointer.y)
                    .is_some()
                {
                    state.seat.pointer.change_state(Some(PointerState::Hover));
                    state.deselect_notification();
                    return;
                }

                if let Some(under_pointer) =
                    state.notifications.get_by_coordinates(pointer.x, pointer.y)
                {
                    let style = under_pointer.style();
                    let mut acc = 0.;
                    state.notifications.iter().find(|notification| {
                        acc += notification.extents().height;
                        notification == &under_pointer
                    });
                    acc -= under_pointer.rendered_extents().height;
                    if under_pointer
                        .text
                        .hit(pointer.x as f32, pointer.y as f32 - acc)
                        .is_some()
                    {
                        state.seat.pointer.change_state(Some(PointerState::Hover));
                        return;
                    }
                }

                state.seat.pointer.change_state(Some(PointerState::Default));

                match (hovered_id, state.notifications.selected()) {
                    (Some(new_id), Some(old_id)) if new_id != old_id => {
                        state.select_notification(new_id);
                        state.update_surface_size();
                    }
                    (Some(new_id), None) => {
                        state.select_notification(new_id);
                        state.update_surface_size();
                    }
                    (None, Some(_)) => {
                        state.deselect_notification();
                        state.update_surface_size();
                    }
                    _ => {}
                }
            }
            wl_pointer::Event::Button {
                serial: _,
                time: _,
                button,
                state: WEnum::Value(value),
            } => {
                if button != LEFT_MOUSE_CLICK {
                    return;
                }

                match value {
                    wl_pointer::ButtonState::Pressed => {
                        state.seat.pointer.change_state(Some(PointerState::Pressed));
                    }
                    wl_pointer::ButtonState::Released => {
                        let (x, y) = (state.seat.pointer.x, state.seat.pointer.y);

                        if let Some(under_pointer) = state.notifications.get_by_coordinates(x, y) {
                            // Get y position of this notification and subtract from y
                            let mut acc = 0.;
                            state.notifications.iter().find(|notification| {
                                acc += notification.extents().height;
                                notification == &under_pointer
                            });
                            acc -= under_pointer.rendered_extents().height;

                            if let Some(anchor) = under_pointer.text.hit(x as f32, y as f32 - acc) {
                                let handle = surface.handle.as_ref().map_or("".into(), Arc::clone);
                                let token = surface.token.as_ref().map(Arc::clone);
                                state.emit_sender.send(EmitEvent::OpenURI {
                                    uri: Arc::clone(&anchor.href),
                                    token,
                                    handle,
                                });
                            }
                            if let Some(button) =
                                state.notifications.get_button_by_coordinates(x, y)
                            {
                                if button.action == Action::DismissNotification {
                                    state.dismiss_notification(under_pointer.id());
                                    state.render();
                                    state.seat.pointer.change_state(Some(PointerState::Default))
                                }
                            }
                        }

                        if state
                            .notifications
                            .get_button_by_coordinates(x, y)
                            .is_some()
                        {
                            state.seat.pointer.change_state(Some(PointerState::Hover));
                        } else {
                            state.seat.pointer.change_state(Some(PointerState::Default))
                        }
                    }
                    _ => unreachable!(),
                }
            }
            wl_pointer::Event::Leave { serial, surface: _ } => {
                state.seat.pointer.wl_pointer.set_cursor(serial, None, 0, 0);
                state.seat.pointer.change_state(None);
            }
            wl_pointer::Event::Enter {
                serial,
                surface: _,
                surface_x,
                surface_y,
            } => {
                surface
                    .layer_surface
                    .set_keyboard_interactivity(KeyboardInteractivity::OnDemand);

                state.seat.pointer.x = surface_x;
                state.seat.pointer.y = surface_y;

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
