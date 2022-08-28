use slog_scope::{debug, warn};
use smithay::delegate_xdg_shell;
use smithay::desktop::PopupKind;

use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

use smithay::input::pointer::GrabStartData;
use smithay::input::Seat;
use smithay::reexports::wayland_server::protocol::wl_seat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::Serial;
use smithay::wayland::shell::xdg::{
    Configure, PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};

use crate::Wazemmes;
use smithay::utils::SERIAL_COUNTER;

impl XdgShellHandler for Wazemmes {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let workspace = self.get_current_workspace();
        let mut workspace = workspace.get_mut();
        debug!("New toplevel window");
        {
            let container = if let Some(layout) = self.next_layout {
                self.next_layout = None;
                workspace.create_container(layout)
            } else {
                workspace.get_focus().0
            };

            {
                let mut container = container.get_mut();
                container.push_toplevel(surface.clone());
            }

            // Grab keyboard focus
            let handle = self
                .seat
                .get_keyboard()
                .expect("Should have a keyboard seat");

            let serial = SERIAL_COUNTER.next_serial();
            handle.set_focus(self, Some(surface.wl_surface().clone()), serial);
            workspace.needs_redraw = true;
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        // Do not send a configure here, the initial configure
        // of a xdg_surface has to be sent during the commit if
        // the surface is not already configured

        // TODO: properly recompute the geometry with the whole of positioner state
        surface.with_pending_state(|state| {
            // NOTE: This is not really necessary as the default geometry
            // is already set the same way, but for demonstrating how
            // to set the initial popup geometry this code is left as
            // an example
            state.geometry = positioner.get_geometry();
        });
        if let Err(err) = self.popups.track_popup(PopupKind::from(surface)) {
            warn!("Failed to track popup: {}", err);
        }
    }

    fn resize_request(
        &mut self,
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

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        // TODO: unimplemented
    }

    // FIXME: redrawing everything on each ack is a bit too much
    fn ack_configure(&mut self, _surface: WlSurface, _configure: Configure) {
        let ws = self.get_current_workspace();
        let mut ws = ws.get_mut();
        ws.update_layout(&self.space);
    }
}

// Xdg Shell
delegate_xdg_shell!(Wazemmes);

fn check_grab(
    seat: &Seat<Wazemmes>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<GrabStartData<Wazemmes>> {
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
