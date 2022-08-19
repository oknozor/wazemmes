use slog_scope::debug;
use smithay::delegate_xdg_shell;

use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

use smithay::reexports::wayland_server::protocol::wl_seat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{DisplayHandle, Resource};
use smithay::wayland::seat::{PointerGrabStartData, Seat};
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::Serial;

use crate::Wazemmes;
use smithay::wayland::SERIAL_COUNTER;

impl XdgShellHandler for Wazemmes {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, dh: &DisplayHandle, surface: ToplevelSurface) {
        let workspace = self.get_current_workspace();
        let mut workspace = workspace.get_mut();
        let space = &mut self.space;

        {
            let container = if let Some(layout) = self.next_layout {
                self.next_layout = None;
                workspace.create_container(layout)
            } else {
                workspace.get_focus().0
            };

            {
                let mut container = container.get_mut();
                container.push_window(surface.clone());
            }

            // Grab keyboard focus
            let handle = self
                .seat
                .get_keyboard()
                .expect("Should have a keyboard seat");

            let serial = SERIAL_COUNTER.next_serial();
            handle.set_focus(dh, Some(surface.wl_surface()), serial);
            let root = workspace.root();
            let mut root = root.get_mut();
            root.redraw(space);
        }
    }

    fn new_popup(
        &mut self,
        _dh: &DisplayHandle,
        _surface: PopupSurface,
        _positioner: PositionerState,
    ) {
    }

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
        let seat: Seat<Wazemmes> = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        if let Some(_start_data) = check_grab(&seat, wl_surface, serial) {
            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            surface.send_configure();
        }
    }

    fn grab(
        &mut self,
        _dh: &DisplayHandle,
        _surface: PopupSurface,
        _seat: wl_seat::WlSeat,
        _serial: Serial,
    ) {
        // TODO popup grabs
    }
}

// Xdg Shell
delegate_xdg_shell!(Wazemmes);

fn check_grab(
    seat: &Seat<Wazemmes>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData> {
    let pointer = seat.get_pointer()?;
    debug!("Check grab");

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

    debug!("Grab detected");
    Some(start_data)
}
