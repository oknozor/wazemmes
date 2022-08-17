use crate::shell::node;
use slog_scope::debug;
use smithay::desktop::{Kind, Space, Window};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point, Size};
use smithay::wayland::shell::xdg::ToplevelSurface;
use std::cell::RefCell;

pub const FLOATING_Z_INDEX: u8 = 255;
pub const TILING_Z_INDEX: u8 = 100;

#[derive(Debug, Clone)]
pub struct WindowState {
    id: RefCell<u32>,
    floating: RefCell<bool>,
    location: RefCell<Point<i32, Logical>>,
}

impl WindowState {
    fn new() -> Self {
        Self {
            id: RefCell::new(node::id::next()),
            floating: RefCell::new(false),
            location: RefCell::new((0, 0).into()),
        }
    }

    pub(crate) fn id(&self) -> u32 {
        *self.id.borrow()
    }

    pub(crate) fn is_floating(&self) -> bool {
        *self.floating.borrow()
    }

    pub(crate) fn location(&self) -> Point<i32, Logical> {
        *self.location.borrow()
    }

    pub fn set_location<P: Into<Point<i32, Logical>>>(&self, location: P) {
        self.location.replace(location.into());
    }

    fn toggle_floating(&self) {
        debug!("Floating toogle for window[{}]", *self.id.borrow());
        let current = *self.floating.borrow();
        self.floating.replace(!current);
    }
}

#[derive(Debug, Clone)]
pub struct WindowWarp {
    inner: Window,
}

impl WindowWarp {
    pub fn get_state(&self) -> &WindowState {
        self.inner.user_data().get::<WindowState>().unwrap()
    }

    pub fn id(&self) -> u32 {
        self.inner.user_data().get::<WindowState>().unwrap().id()
    }

    pub fn get(&self) -> &Window {
        &self.inner
    }

    pub fn toplevel(&self) -> &ToplevelSurface {
        match self.inner.toplevel() {
            Kind::Xdg(toplevel) => toplevel,
            Kind::X11(_) => unimplemented!(),
        }
    }

    pub fn configure<S: Into<Size<i32, Logical>>>(
        &self,
        space: &mut Space,
        size: S,
        activate: bool,
    ) {
        let toplevel = self.toplevel();
        let location = self.inner.bbox().loc;

        toplevel.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Resizing);
            state.size = Some(size.into())
        });

        toplevel.send_configure();
        space.map_window(&self.inner, location, None, activate);
    }

    pub fn send_close(&self) {
        self.toplevel().send_close()
    }

    pub fn toggle_floating(&mut self) {
        self.get_state().toggle_floating();
    }

    pub fn is_floating(&self) -> bool {
        self.get_state().is_floating()
    }

    pub fn location(&self) -> Point<i32, Logical> {
        self.get_state().location()
    }

    pub fn z_index(&self) -> u8 {
        if self.is_floating() {
            FLOATING_Z_INDEX
        } else {
            TILING_Z_INDEX
        }
    }
}

impl From<ToplevelSurface> for WindowWarp {
    fn from(toplevel: ToplevelSurface) -> Self {
        let window = Window::new(Kind::Xdg(toplevel));
        window.user_data().insert_if_missing(WindowState::new);

        WindowWarp { inner: window }
    }
}

impl From<Window> for WindowWarp {
    fn from(window: Window) -> Self {
        WindowWarp { inner: window }
    }
}
