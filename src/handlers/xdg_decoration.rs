use smithay::delegate_xdg_decoration;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::shell::xdg::decoration::{XdgDecorationHandler};
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
use crate::Wazemmes;

impl XdgDecorationHandler for Wazemmes {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });

        toplevel.send_configure();
    }

    fn request_mode(&mut self, _toplevel: ToplevelSurface, _mode: Mode) {
        // Unused
    }

    fn unset_mode(&mut self, _toplevel: ToplevelSurface) {
        // Unused
    }
}

delegate_xdg_decoration!(Wazemmes);
