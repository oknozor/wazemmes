use std::sync::Mutex;
use smithay::{
    delegate_xdg_shell,
    desktop::{Kind, Space, WindowSurfaceType},
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface},
            DisplayHandle, Resource,
        },
    },
    wayland::{
        compositor::with_states,
        seat::{PointerGrabStartData, Seat},
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceRoleAttributes,
        },
        Serial,
    },
};
use smithay::wayland::SERIAL_COUNTER;
use crate::{Tree, Wazemmes};

#[derive(Debug, Copy, Clone)]
struct WindowId(u32);

impl XdgShellHandler for Wazemmes {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, dh: &DisplayHandle, surface: ToplevelSurface) {
        if self.tree.is_none() {
            let output = self.space.outputs().next().unwrap();
            let geo = self.space.output_geometry(&output).unwrap();
            self.tree = Some(Tree::new(&output, geo))
        }

        let container = if let Some(layout) = self.next_layout {
            self.next_layout = None;
            self.tree().create_container(layout)
        } else {
            self.tree().get_container_focused()
        };

        let mut container = container.borrow_mut();
        container.push_window(surface.clone(), &mut self.space);

        let handle = self
            .seat
            .get_keyboard()
            .expect("Should have a keyboard seat");

        let serial = SERIAL_COUNTER.next_serial();
        handle.set_focus(&dh, Some(&surface.wl_surface()), serial);
    }

    fn new_popup(&mut self, _dh: &DisplayHandle, _surface: PopupSurface, _positioner: PositionerState) {}

    fn move_request(
        &mut self,
        _dh: &DisplayHandle,
        _surface: ToplevelSurface,
        _seat: wl_seat::WlSeat,
        _serial: Serial,
    ) {

    }

    fn resize_request(
        &mut self,
        _dh: &DisplayHandle,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        _edges: xdg_toplevel::ResizeEdge,
    ) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        if let Some(_start_data) = check_grab(&seat, wl_surface, serial) {
            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            println!("resize ?");
            surface.send_configure();
        }
    }

    fn grab(&mut self, _dh: &DisplayHandle, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        // TODO popup grabs
    }
}

// Xdg Shell
delegate_xdg_shell!(Wazemmes);

fn check_grab(seat: &Seat<Wazemmes>, surface: &WlSurface, serial: Serial) -> Option<PointerGrabStartData> {
    let pointer = seat.get_pointer()?;

    // Check that this surface has a click grab.
    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;

    // If the focus was for a different surface, ignore the request.
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}

/// Should be called on `WlSurface::commit`
pub fn handle_commit(space: &Space, surface: &WlSurface) -> Option<()> {
    let window = space
        .window_for_surface(surface, WindowSurfaceType::TOPLEVEL)
        .cloned()?;

    if let Kind::Xdg(_) = window.toplevel() {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });

        if !initial_configure_sent {
            window.configure();
        }
    }

    Some(())
}