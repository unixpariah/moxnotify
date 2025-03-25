use crate::{button::ButtonType, surface::FocusReason, EmitEvent, Moxnotify};
use std::sync::Arc;
use wayland_client::{
    delegate_noop,
    globals::GlobalList,
    protocol::{wl_pointer, wl_seat},
    Connection, Dispatch, QueueHandle, WEnum,
};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{self, Shape},
    wp_cursor_shape_manager_v1,
};

#[derive(PartialEq, Debug)]
enum PointerState {
    Pressed,
    Default,
    Hover,
}

pub struct Pointer {
    state: PointerState,
    x: f64,
    y: f64,
    _wl_pointer: wl_pointer::WlPointer,
    scroll_accumulator: f64,
    cursor_device: wp_cursor_shape_device_v1::WpCursorShapeDeviceV1,
    serial: u32,
}

delegate_noop!(Moxnotify: wp_cursor_shape_manager_v1::WpCursorShapeManagerV1);
delegate_noop!(Moxnotify: wp_cursor_shape_device_v1::WpCursorShapeDeviceV1);

impl Pointer {
    pub fn new(
        qh: &QueueHandle<Moxnotify>,
        globals: &GlobalList,
        wl_seat: &wl_seat::WlSeat,
    ) -> anyhow::Result<Self> {
        let wl_pointer = wl_seat.get_pointer(qh, ());
        let cursor_shape = globals
            .bind::<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1, _, _>(qh, 1..=1, ())?;
        let cursor_device = cursor_shape.get_pointer(&wl_pointer, qh, ());

        Ok(Self {
            serial: 0,
            cursor_device,
            state: PointerState::Default,
            x: 0.,
            y: 0.,
            _wl_pointer: wl_pointer,
            scroll_accumulator: 0.,
        })
    }

    fn change_state(&mut self, pointer_state: PointerState) {
        match pointer_state {
            PointerState::Default => {
                self.cursor_device.set_shape(self.serial, Shape::Default);
            }
            PointerState::Pressed => {}
            PointerState::Hover => {
                self.cursor_device.set_shape(self.serial, Shape::Pointer);
            }
        }

        self.state = pointer_state;
    }
}

const LEFT_MOUSE_CLICK: u32 = 272;

impl Dispatch<wl_pointer::WlPointer, ()> for Moxnotify {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: <wl_pointer::WlPointer as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                let hovered_id = state
                    .notifications
                    .get_by_coordinates(surface_x, surface_y)
                    .map(|n| n.id());

                let pointer = &mut state.seat.pointer;
                pointer.x = surface_x;
                pointer.y = surface_y;

                if let PointerState::Pressed = pointer.state {
                    return;
                }

                let pointer = &state.seat.pointer;
                if state
                    .notifications
                    .get_button_by_coordinates(pointer.x, pointer.y)
                    .is_some()
                {
                    state.seat.pointer.change_state(PointerState::Hover);
                } else {
                    state.seat.pointer.change_state(PointerState::Default);
                }

                let pointer = &state.seat.pointer;
                if let Some(under_pointer) =
                    state.notifications.get_by_coordinates(pointer.x, pointer.y)
                {
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
                        state.seat.pointer.change_state(PointerState::Hover);
                        return;
                    }
                }

                match (hovered_id, state.notifications.selected()) {
                    (Some(new_id), Some(old_id)) if new_id != old_id => {
                        state.notifications.select(new_id);
                        state.update_surface_size();
                    }
                    (Some(new_id), None) => {
                        state.notifications.select(new_id);
                        state.update_surface_size();
                    }
                    (None, Some(_)) => {
                        state.notifications.deselect();
                        state.update_surface_size();
                    }
                    _ => {}
                }
            }
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(value),
                ..
            } => {
                if button != LEFT_MOUSE_CLICK {
                    return;
                }

                match value {
                    wl_pointer::ButtonState::Pressed => {
                        state.seat.pointer.change_state(PointerState::Pressed);
                    }
                    wl_pointer::ButtonState::Released => {
                        state.seat.pointer.change_state(PointerState::Default);

                        let (x, y) = (state.seat.pointer.x, state.seat.pointer.y);

                        let (href, notification_id, button) = {
                            if let Some(under_pointer) =
                                state.notifications.get_by_coordinates(x, y)
                            {
                                let notification_id = under_pointer.id();

                                let mut acc = 0.0;
                                let _ = state.notifications.iter().find(|n| {
                                    acc += n.extents().height;
                                    n == &under_pointer
                                });
                                acc -= under_pointer.rendered_extents().height;

                                let href = under_pointer
                                    .text
                                    .hit(x as f32, y as f32 - acc)
                                    .map(|anchor| Arc::clone(&anchor.href));

                                let button = state.notifications.get_button_by_coordinates(x, y);

                                (href, Some(notification_id), button)
                            } else {
                                (None, None, None)
                            }
                        };

                        if let Some(href) = href {
                            if let Some(surface) = state.surface.as_ref() {
                                let token = surface.token.as_ref().map(Arc::clone);
                                if state
                                    .emit_sender
                                    .send(EmitEvent::Open { uri: href, token })
                                    .is_ok()
                                {
                                    state.notifications.deselect();
                                }
                            }
                        }

                        if let Some(notification_id) = notification_id {
                            match button {
                                Some(ButtonType::Dismiss) => {
                                    state.notifications.dismiss(notification_id);
                                }
                                Some(ButtonType::Action { action, .. }) => {
                                    if !state
                                        .notifications
                                        .iter()
                                        .find(|notification| notification.id() == notification_id)
                                        .map(|n| n.hints.resident)
                                        .unwrap_or_default()
                                    {
                                        state.notifications.dismiss(notification_id);
                                    }

                                    if let Some(surface) = state.surface.as_ref() {
                                        let token = surface.token.as_ref().map(Arc::clone);
                                        _ = state.emit_sender.send(EmitEvent::ActionInvoked {
                                            id: notification_id,
                                            action_key: action,
                                            token: token.unwrap_or_default(),
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }

                        if let Some(notification) = state.notifications.get_by_coordinates(x, y) {
                            state.notifications.select(notification.id());
                        }

                        if state
                            .notifications
                            .get_button_by_coordinates(x, y)
                            .is_some()
                        {
                            state.seat.pointer.change_state(PointerState::Hover);
                        }
                    }
                    _ => unreachable!(),
                }
            }
            wl_pointer::Event::Leave { .. } => {
                if let Some(surface) = state.surface.as_mut() {
                    if surface.focus_reason == Some(FocusReason::MouseEnter) {
                        surface.unfocus();
                        state.seat.pointer.change_state(PointerState::Default);
                        state.notifications.deselect();
                    }
                }
            }
            wl_pointer::Event::Enter {
                serial,
                surface_x,
                surface_y,
                ..
            } => {
                state.seat.pointer.serial = serial;

                if let Some(surface) = state.surface.as_mut() {
                    surface.focus(FocusReason::MouseEnter)
                }

                state.seat.pointer.x = surface_x;
                state.seat.pointer.y = surface_y;

                state.seat.pointer.change_state(PointerState::Default);
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

                        state.seat.pointer.scroll_accumulator = 0.0;
                    }
                }
            }
            _ => {}
        }

        state.update_surface_size();
        if let Some(surface) = state.surface.as_mut() {
            _ = surface.render(
                &state.wgpu_state.device,
                &state.wgpu_state.queue,
                &state.notifications,
            );
        }
    }
}
