use smithay::desktop::{Kind, Space, Window};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Size};
use smithay::wayland::shell::xdg::ToplevelSurface;

#[derive(Debug, Copy, Clone)]
pub struct WindowId(u32);

impl WindowId {
    pub fn get(&self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct WindowWarp {
    inner: Window,
}

impl WindowWarp {
    pub fn toplevel(&self) -> &ToplevelSurface {
        match self.inner.toplevel() {
            Kind::Xdg(toplevel) => toplevel,
            Kind::X11(_) => unimplemented!(),
        }
    }

    pub fn id(&self) -> u32 {
        self.inner.user_data().get::<WindowId>().unwrap().0
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
        window
            .user_data()
            .insert_if_missing(|| WindowId(id::next()));

        WindowWarp { inner: window }
    }
}

pub mod id {
    use once_cell::sync::Lazy;
    use std::sync::{Arc, Mutex};

    static WINDOW_ID_COUNTER: Lazy<Arc<Mutex<u32>>> = Lazy::new(|| Arc::new(Mutex::new(0)));

    pub fn next() -> u32 {
        let mut id = WINDOW_ID_COUNTER.lock().unwrap();
        *id += 1;
        *id
    }
}
