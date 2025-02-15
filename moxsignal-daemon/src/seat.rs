mod keyboard;
mod pointer;

use crate::Moxsignal;
use keyboard::Keyboard;
use pointer::Pointer;
use wayland_client::{
    delegate_noop,
    globals::GlobalList,
    protocol::{wl_compositor, wl_seat, wl_shm},
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1;

pub struct Seat {
    name: Option<Box<str>>,
    pub wl_seat: wl_seat::WlSeat,
    pointer: Pointer,
    pub keyboard: Keyboard,
    pub xdg_activation: xdg_activation_v1::XdgActivationV1,
}

impl Seat {
    pub fn new(
        conn: &Connection,
        qh: &QueueHandle<Moxsignal>,
        globals: &GlobalList,
        compositor: &wl_compositor::WlCompositor,
    ) -> anyhow::Result<Self> {
        let wl_seat = globals.bind::<wl_seat::WlSeat, _, _>(qh, 1..=4, ())?;
        let keyboard = Keyboard::new(qh, &wl_seat);
        let pointer = Pointer::new(conn, qh, compositor, globals, &wl_seat)?;

        Ok(Self {
            xdg_activation: globals.bind(qh, 1..=1, ())?,
            name: None,
            wl_seat,
            pointer,
            keyboard,
        })
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Moxsignal {
    fn event(
        state: &mut Self,
        _proxy: &wl_seat::WlSeat,
        event: <wl_seat::WlSeat as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Name { name } = event {
            state.seat.name = Some(name.into())
        }
    }
}

delegate_noop!(Moxsignal: ignore wl_shm::WlShm);
