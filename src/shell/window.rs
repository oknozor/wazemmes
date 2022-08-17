use crate::shell::node;
use smithay::desktop::{Kind, Space, Window};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Size};
use smithay::wayland::shell::xdg::ToplevelSurface;
use std::cell::RefCell;

#[derive(Debug, Clone)]
pub struct WindowState {
    id: RefCell<u32>,
    floating: RefCell<bool>,
}

impl WindowState {
    fn new() -> Self {
        Self {
            id: RefCell::new(node::id::next()),
            floating: RefCell::new(false),
        }
    }

    pub(crate) fn id(&self) -> u32 {
        *self.id.borrow()
    }

    pub(crate) fn is_floating(&self) -> bool {
        *self.floating.borrow()
    }

    fn toggle_floating(&self) {
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

    pub fn toplevel(&self) -> &ToplevelSurface {
        match self.inner.toplevel() {
            Kind::Xdg(toplevel) => toplevel,
            Kind::X11(_) => unimplemented!(),
        }
    }

    pub fn id(&self) -> u32 {
        self.inner.user_data().get::<WindowState>().unwrap().id()
    }

    pub fn get(&self) -> &Window {
        &self.inner
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

    pub(crate) fn new(toplevel: ToplevelSurface) -> Self {
        let window = Window::new(Kind::Xdg(toplevel));
        window.user_data().insert_if_missing(WindowState::new);

        WindowWarp { inner: window }
    }

    pub fn toggle_floating(&mut self) {
        self.get_state().toggle_floating();
    }

    pub fn is_floating(&self) -> bool {
        self.get_state().is_floating()
    }
}
