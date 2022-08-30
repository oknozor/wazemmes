use crate::backend::xwayland::window::WinType;
use crate::backend::xwayland::X11State;
use slog_scope::{warn};
use smithay::utils::{Logical, Point, Size};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask, Window};
use crate::backend::xwayland::error::XWaylandError;

pub type MoveResizeWindowFlags = u32;

pub const MOVE_RESIZE_WINDOW_X: MoveResizeWindowFlags = 1 << 8;
pub const MOVE_RESIZE_WINDOW_Y: MoveResizeWindowFlags = 1 << 9;
pub const MOVE_RESIZE_WINDOW_WIDTH: MoveResizeWindowFlags = 1 << 10;
pub const MOVE_RESIZE_WINDOW_HEIGHT: MoveResizeWindowFlags = 1 << 11;

impl X11State {
    pub fn send_configure<S>(&self, id: u32, size: Option<S>) -> Result<(), XWaylandError>
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
            .send_event(false, self.root, mask, &message)?
            .check()?;

        self.conn.flush()?;

        Ok(())
    }

    pub fn send_close(&self, id: u32) -> Result<(), XWaylandError>{
        if let Some(id) = self.id_map.get(&id) {
            let message =
                ClientMessageEvent::new(32, *id, self.atoms._NET_CLOSE_WINDOW, [0, 0, 0, 0, 0]);
            let mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;
            self
                .conn
                .send_event(false, self.root, mask, &message)
                .unwrap()
                .check()?;

            self.conn.flush()?;
        } else {
            warn!("Trying to close xwindow({id}) but there is no such window")
        }

        Ok(())
    }

    pub fn get_window_type(&self, xwindow: Window) -> Result<WinType, XWaylandError> {
        let reply = self
            .conn
            .get_property(
                false,
                xwindow,
                self.atoms._NET_WM_WINDOW_TYPE,
                AtomEnum::ATOM,
                0,
                u32::MAX,
            )?.reply()?;

        if let Some(typ) = reply.value32().and_then(|mut x| x.next()) {
            WinType::from(&self.atoms, typ)
        } else {
            Err(XWaylandError::EmptyReply)
        }
    }

    pub fn get_parent(&self, xwindow: u32) -> Result<Window, XWaylandError> {
        let parent = self.conn.query_tree(xwindow)?.reply()?.parent;
        Ok(parent)
    }

    pub fn get_location(&self, window: Window) -> Result<Point<i32, Logical>, XWaylandError> {
        let loc = self.conn.get_geometry(window)?.reply()?;
        Ok((loc.x as i32, loc.y as i32).into())
    }
}
