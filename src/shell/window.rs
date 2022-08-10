use smithay::desktop::{Kind, Window};
use smithay::wayland::shell::xdg::ToplevelSurface;

#[derive(Debug, Clone)]
pub struct WindowWarp(Window);

impl WindowWarp {
    pub fn get_toplevel(&self) -> &ToplevelSurface {
        match self.0.toplevel() {
            Kind::Xdg(toplevel) => toplevel,
            Kind::X11(_) => unimplemented!(),
        }
    }

    pub fn get(&self) -> &Window {
        &self.0
    }

    pub fn send_close(&self) {
        self.get_toplevel().send_close()
    }
}

impl From<ToplevelSurface> for WindowWarp {
    fn from(toplevel: ToplevelSurface) -> Self {
        WindowWarp(Window::new(Kind::Xdg(toplevel)))
    }
}
