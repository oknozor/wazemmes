use slog_scope::{debug, error, warn};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use crate::backend::xwayland::window::WinType;
use crate::shell::window::WindowWrap;
use crate::shell::x11_popup::X11Popup;
use crate::{Wazemmes, WorkspaceRef};
use smithay::desktop::{Kind, Window, X11Surface};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DisplayHandle, Resource};
use smithay::utils::x11rb::X11Source;
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::give_role;
use x11rb::connection::Connection;
use x11rb::errors::ReplyOrIdError;
use x11rb::protocol::composite::{ConnectionExt as _, Redirect};
use x11rb::protocol::xproto::{
    ChangeWindowAttributesAux, ConfigWindow, ConfigureWindowAux, ConnectionExt as _, EventMask,
    StackMode, Window as X11Window, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::{DefaultStream, RustConnection};

mod client;
mod window;

impl Wazemmes {
    #[cfg(feature = "xwayland")]
    pub fn start_xwayland(&mut self) {
        debug!("Starting xwayland");
        if let Err(e) = self.xwayland.start(self._loop_handle.clone()) {
            error!("Failed to start XWayland: {}", e);
        }

        debug_assert!(std::env::var("DISPLAY").is_ok())
    }

    pub fn xwayland_ready(&mut self, connection: UnixStream, client: Client) {
        let (wm, source) = X11State::start_wm(connection, client).unwrap();
        self.x11_state = Some(wm);
        let workspace_ref = self.get_current_workspace();

        self._loop_handle
            .insert_source(source, move |event, _, data| {
                if let Some(x11) = data.state.x11_state.as_mut() {
                    match x11.handle_event(event, &data.display.handle(), workspace_ref.clone()) {
                        Ok(()) => {}
                        Err(err) => error!("Error while handling X11 event: {}", err),
                    }
                }
            })
            .unwrap();
    }

    pub fn xwayland_exited(&mut self) {
        let _ = self.x11_state.take();
        error!("Xwayland crashed");
    }
}

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        WM_S0,
        WL_SURFACE_ID,
        _WAZEMMES_CLOSE_CONNECTION,
        _NET_MOVERESIZE_WINDOW,
        _NET_CLOSE_WINDOW,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DESKTOP,
        _NET_WM_WINDOW_TYPE_DOCK,
        _NET_WM_WINDOW_TYPE_TOOLBAR,
        _NET_WM_WINDOW_TYPE_MENU,
        _NET_WM_WINDOW_TYPE_UTILITY,
        _NET_WM_WINDOW_TYPE_SPLASH,
        _NET_WM_WINDOW_TYPE_DIALOG,
        _NET_WM_WINDOW_TYPE_NORMAL,
        _NET_WM_WINDOW_TYPE_COMBO,
        _NET_WM_WINDOW_TYPE_DND,
        _NET_WM_WINDOW_TYPE_NOTIFICATION,
        _NET_WM_WINDOW_TYPE_POPUP_MENU,
        _NET_WM_WINDOW_TYPE_TOOLTIP,
        _NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
    }
}

/// The actual runtime state of the XWayland integration.
#[derive(Debug)]
pub struct X11State {
    conn: Arc<RustConnection>,
    atoms: Atoms,
    client: Client,
    unpaired_surfaces: HashMap<u32, (X11Window, Point<i32, Logical>)>,
    id_map: HashMap<u32, u32>,
    root: x11rb::protocol::xproto::Window,
    pub needs_redraw: bool,
}

impl X11State {
    fn start_wm(
        connection: UnixStream,
        client: Client,
    ) -> Result<(Self, X11Source), Box<dyn std::error::Error>> {
        // Create an X11 connection. XWayland only uses screen 0.
        let screen = 0;
        let stream = DefaultStream::from_unix_stream(connection)?;
        let conn = RustConnection::connect_to_stream(stream, screen)?;
        let atoms = Atoms::new(&conn)?.reply()?;

        let screen = &conn.setup().roots[0];

        // Actually become the WM by redirecting some operations
        let root = screen.root;
        conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::default().event_mask(EventMask::SUBSTRUCTURE_REDIRECT),
        )?;

        // Tell XWayland that we are the WM by acquiring the WM_S0 selection. No X11 clients are accepted before this.
        let win = conn.generate_id()?;
        conn.create_window(
            screen.root_depth,
            win,
            root,
            // x, y, width, height, border width
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &Default::default(),
        )?;

        conn.set_selection_owner(win, atoms.WM_S0, x11rb::CURRENT_TIME)?;

        // XWayland wants us to do this to function properly...?
        conn.composite_redirect_subwindows(root, Redirect::MANUAL)?;

        conn.flush()?;

        let conn = Arc::new(conn);

        let wm = Self {
            conn: Arc::clone(&conn),
            atoms,
            client,
            unpaired_surfaces: Default::default(),
            id_map: Default::default(),
            root,
            needs_redraw: false,
        };

        Ok((
            wm,
            X11Source::new(
                conn,
                win,
                atoms._WAZEMMES_CLOSE_CONNECTION,
                slog_scope::logger(),
            ),
        ))
    }

    fn handle_event(
        &mut self,
        event: Event,
        dh: &DisplayHandle,
        ws: WorkspaceRef,
    ) -> Result<(), ReplyOrIdError> {
        debug!("X11: Got event {:?}", event);
        self.needs_redraw = false;

        match event {
            Event::ConfigureRequest(r) => {
                // Just grant the wish
                let mut aux = ConfigureWindowAux::default();
                if r.value_mask & u16::from(ConfigWindow::STACK_MODE) != 0 {
                    aux = aux.stack_mode(r.stack_mode);
                }
                if r.value_mask & u16::from(ConfigWindow::SIBLING) != 0 {
                    aux = aux.sibling(r.sibling);
                }
                if r.value_mask & u16::from(ConfigWindow::X) != 0 {
                    aux = aux.x(i32::try_from(r.x).unwrap());
                }
                if r.value_mask & u16::from(ConfigWindow::Y) != 0 {
                    aux = aux.y(i32::try_from(r.y).unwrap());
                }
                if r.value_mask & u16::from(ConfigWindow::WIDTH) != 0 {
                    aux = aux.width(u32::try_from(r.width).unwrap());
                }
                if r.value_mask & u16::from(ConfigWindow::HEIGHT) != 0 {
                    aux = aux.height(u32::try_from(r.height).unwrap());
                }
                if r.value_mask & u16::from(ConfigWindow::BORDER_WIDTH) != 0 {
                    aux = aux.border_width(u32::try_from(r.border_width).unwrap());
                }

                println!("SIBLINGS {}", r.sibling);

                self.conn.configure_window(r.window, &aux)?;
            }
            Event::MapRequest(r) => {
                // Just grant the wish
                debug!("XMap request for window={} parent={}", r.window, r.parent);
                self.conn.map_window(r.window)?;
            }
            Event::ClientMessage(msg) => {
                if msg.type_ == self.atoms._NET_MOVERESIZE_WINDOW {
                    debug!("Got X Client message: _NET_MOVERESIZE_WINDOW");
                    let data = msg.data.as_data32();
                    let [_, _, _, w, h] = data;
                    let id = self.id_map.get(&msg.window).unwrap();
                    let aux = ConfigureWindowAux {
                        x: Some(0),
                        y: Some(0),
                        width: Some(w),
                        height: Some(h),
                        border_width: Some(1),
                        sibling: None,
                        stack_mode: Some(StackMode::ABOVE),
                    };
                    self.conn.configure_window(*id, &aux)?;
                    self.conn.flush()?;
                    self.needs_redraw = true;
                } else if msg.type_ == self.atoms._NET_CLOSE_WINDOW {
                    debug!("Got X Client message: _NET_CLOSE_WINDOW");
                    // TODO: how do we correctly terminate the process here ?
                    self.conn.destroy_window(msg.window)?;
                    self.conn.flush()?;
                    self.needs_redraw = true;

                    let window_ids = self
                        .id_map
                        .iter()
                        .find(|(_wl_id, x_id)| **x_id == msg.window)
                        .map(|(w_id, x_id)| (*w_id, *x_id));

                    if let Some((wl_id, _x_id)) = window_ids {
                        self.id_map.remove(&wl_id);
                    };
                } else if msg.type_ == self.atoms.WL_SURFACE_ID {
                    debug!("Got X Client message: WL_SURFACE_ID");
                    // We get a WL_SURFACE_ID message when Xwayland creates a WlSurface for a
                    // window. Both the creation of the surface and this client message happen at
                    // roughly the same time and are sent over different sockets (X11 socket and
                    // wayland socket). Thus, we could receive these two in any order. Hence, it
                    // can happen that we get None below when X11 was faster than Wayland.

                    let location = {
                        match self.conn.get_geometry(msg.window)?.reply() {
                            Ok(geo) => (geo.x as i32, geo.y as i32).into(),
                            Err(err) => {
                                error!(
                                    "Failed to get geometry for {:x}, perhaps the window was already destroyed?",
                                    msg.window;
                                    "err" => format!("{:?}", err),
                                );
                                (0, 0).into()
                            }
                        }
                    };
                    let id = msg.data.as_data32()[0];
                    let surface = self.client.object_from_protocol_id(dh, id);

                    match surface {
                        Err(err) => {
                            warn!("X11 surface has invalid id {:?}", err);
                            self.unpaired_surfaces.insert(id, (msg.window, location));
                        }
                        Ok(surface) => {
                            debug!(
                                "X11 surface {:x?} corresponds to WlSurface {:x} = {:?}",
                                msg.window, id, surface,
                            );
                            self.new_window(msg.window, surface, ws);
                        }
                    }
                }
            }
            _ => {}
        }

        self.conn.flush()?;
        Ok(())
    }

    fn new_window(&mut self, xwindow: X11Window, surface: WlSurface, ws: WorkspaceRef) {
        debug!("Matched X11 surface {:x?} to {:x?}", xwindow, surface);

        let win_type = self.get_window_type(xwindow);

        if give_role(&surface, "x11_surface").is_err() {
            error!("Surface {:x?} already has a role?!", surface);
            return;
        }
        let protocol_id = surface.id().protocol_id();
        let x11surface = X11Surface { surface };
        let ws = ws.get();
        let (container, _window) = ws.get_focus();
        let mut container = container.get_mut();
        self.id_map.insert(protocol_id, xwindow);

        match win_type {
            WinType::Normal => {
                debug!("New toplevel from XWindow {xwindow}");
                let window = WindowWrap::from_x11_window(Window::new(Kind::X11(x11surface)));
                container.push_xwindow(window);
            }
            _ => {
                let popup = Window::new(Kind::X11(x11surface));
                let loc = self.get_location(xwindow);
                let parent = self.get_parent(xwindow);
                debug!("xpopup_id={:?}, parent={}", loc, parent);
                container.push_xpopup(X11Popup::new(popup, loc));
            }
        }

        self.needs_redraw = true;
    }
}

// Called when a WlSurface commits.
pub fn commit_hook(
    surface: &WlSurface,
    dh: &DisplayHandle,
    state: &mut X11State,
    ws: WorkspaceRef,
) {
    if let Ok(client) = dh.get_client(surface.id()) {
        // Is this the Xwayland client?
        if client == state.client {
            // Is the surface among the unpaired surfaces (see comment next to WL_SURFACE_ID
            // handling above)
            if let Some((window, _loc)) =
                state.unpaired_surfaces.remove(&surface.id().protocol_id())
            {
                state.new_window(window, surface.clone(), ws);
            }
        }
    }
}
