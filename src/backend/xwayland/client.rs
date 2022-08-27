use crate::backend::xwayland::window::WinType;
use crate::backend::xwayland::X11State;
use slog_scope::{debug, error};
use smithay::utils::{Logical, Point, Size};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window};

pub type MoveResizeWindowFlags = u32;

pub const MOVE_RESIZE_WINDOW_X: MoveResizeWindowFlags = 1 << 8;
pub const MOVE_RESIZE_WINDOW_Y: MoveResizeWindowFlags = 1 << 9;
pub const MOVE_RESIZE_WINDOW_WIDTH: MoveResizeWindowFlags = 1 << 10;
pub const MOVE_RESIZE_WINDOW_HEIGHT: MoveResizeWindowFlags = 1 << 11;

impl X11State {
    pub fn send_configure<S>(&self, id: u32, size: Option<S>)
    where
        S: Into<Size<i32, Logical>>,
    {
        let mut flags = 0;
        let size = size.map(S::into).map(|size| (size.w as u32, size.h as u32));
        let w = size.map(|s| s.0);
        let h = size.map(|s| s.1);
        let (x, y, w, h) = (None, None, w, h);

        // Define the second byte of the move resize flags 32bit value
        // Used to indicate that the associated value has been changed and needs to be acted upon
        if x.is_some() {
            flags |= MOVE_RESIZE_WINDOW_X;
        }
        if y.is_some() {
            flags |= MOVE_RESIZE_WINDOW_Y;
        }
        if w.is_some() {
            flags |= MOVE_RESIZE_WINDOW_WIDTH;
        }
        if h.is_some() {
            flags |= MOVE_RESIZE_WINDOW_HEIGHT;
        }

        let data = [
            flags,
            x.unwrap_or(0),
            y.unwrap_or(0),
            w.unwrap_or(0),
            h.unwrap_or(0),
        ];

        let message = ClientMessageEvent::new(32, id, self.atoms._NET_MOVERESIZE_WINDOW, data);
        let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
        let reply = self
            .conn
            .send_event(false, self.root, mask, &message)
            .unwrap()
            .check();

        self.conn.flush().unwrap();

        if let Err(err) = reply {
            error!("Error sending resize event {err}");
        }
    }

    // TODO: handle error (for instance we don't have the window id)
    pub fn send_close(&self, id: u32) {
        let id = self.id_map.get(&id);
        if let Some(id) = id {
            let message =
                ClientMessageEvent::new(32, *id, self.atoms._NET_CLOSE_WINDOW, [0, 0, 0, 0, 0]);
            let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
            let reply = self
                .conn
                .send_event(false, self.root, mask, &message)
                .unwrap()
                .check();

            self.conn.flush().unwrap();

            if let Err(err) = reply {
                error!("Error sending close event {err}");
            }
        }
    }

    pub fn get_window_type(&self, xwindow: Window) -> WinType {
        let reply = self
            .conn
            .get_property(
                false,
                xwindow,
                self.atoms._NET_WM_WINDOW_TYPE,
                AtomEnum::ATOM,
                0,
                u32::MAX,
            )
            .unwrap()
            .reply()
            .unwrap();
        let typ = reply.value32().and_then(|mut x| x.next()).unwrap();
        let typ = WinType::from(&self.atoms, typ).unwrap();
        debug!("win_type: id: {}, type: {:?}", xwindow, typ);
        typ
    }

    pub fn get_parent(&self, xwindow: u32) -> Window {
        let response = self.conn.query_tree(xwindow).unwrap().reply().unwrap();

        response.parent
    }

    pub fn get_location(&self, window: Window) -> Point<i32, Logical> {
        let loc = self.conn.get_geometry(window).unwrap().reply().unwrap();
        (loc.x as i32, loc.y as i32).into()
    }
}
