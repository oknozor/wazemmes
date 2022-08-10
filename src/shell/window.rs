use smithay::desktop::{Kind, Window};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Size};
use smithay::wayland::shell::xdg::ToplevelSurface;

#[derive(Debug, Clone)]
pub struct WindowWarp {
    inner: Window,
    location: (i32, i32),
}

impl WindowWarp {
    pub fn get_toplevel(&self) -> &ToplevelSurface {
        match self.inner.toplevel() {
            Kind::Xdg(toplevel) => toplevel,
            Kind::X11(_) => unimplemented!(),
        }
    }

    pub fn get(&self) -> &Window {
        &self.inner
    }

    pub fn resize<T: Into<Size<i32, Logical>>>(&self, size: T) {
        self.get_toplevel().with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Resizing);
            state.size = Some(size.into())
        });
    }

    pub fn send_close(&self) {
        self.get_toplevel().send_close()
    }
}

impl From<ToplevelSurface> for WindowWarp {
    fn from(toplevel: ToplevelSurface) -> Self {
        WindowWarp {
            inner: Window::new(Kind::Xdg(toplevel)),
            location: (0, 0),
        }
    }
}
