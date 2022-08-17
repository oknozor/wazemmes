use crate::shell::node;
use slog_scope::debug;
use smithay::desktop::{Kind, Space, Window};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Physical, Point, Rectangle, Size};
use smithay::wayland::shell::xdg::ToplevelSurface;
use std::cell::RefCell;
use smithay::wayland::output::Output;

pub const FLOATING_Z_INDEX: u8 = 255;
pub const TILING_Z_INDEX: u8 = 100;

#[derive(Debug, Clone)]
pub struct WindowState {
    id: RefCell<u32>,
    floating: RefCell<bool>,
    location: RefCell<Point<i32, Logical>>,
    size: RefCell<Size<i32, Logical>>,
}

impl WindowState {
    fn new() -> Self {
        Self {
            id: RefCell::new(node::id::next()),
            floating: RefCell::new(false),
            location: RefCell::new((0, 0).into()),
            size: Default::default(),
        }
    }

    pub fn id(&self) -> u32 {
        *self.id.borrow()
    }

    pub fn is_floating(&self) -> bool {
        *self.floating.borrow()
    }

    pub fn location(&self) -> Point<i32, Logical> {
        *self.location.borrow()
    }

    pub fn set_location<P: Into<Point<i32, Logical>>>(&self, location: P) {
        self.location.replace(location.into());
    }

    pub fn size(&self) -> Size<i32, Logical> {
        *self.size.borrow()
    }

    pub fn set_size<S: Into<Size<i32, Logical>>>(&self, size: S) {
        self.size.replace(size.into());
    }

    fn toggle_floating(&self) {
        debug!("Floating toogle for window[{}]", *self.id.borrow());
        let current = *self.floating.borrow();
        self.floating.replace(!current);
    }
}

#[derive(Debug, Clone)]
pub struct WindowWrap {
    inner: Window,
}

#[derive(Debug, Clone)]
pub struct WindowBorder {
    pub left: Rectangle<i32, Logical>,
    pub right: Rectangle<i32, Logical>,
    pub top: Rectangle<i32, Logical>,
    pub bottom: Rectangle<i32, Logical>,
}

impl WindowWrap {
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

    pub fn toggle_floating(&self) {
        self.get_state().toggle_floating();
    }

    pub fn is_floating(&self) -> bool {
        self.get_state().is_floating()
    }

    pub fn location(&self) -> Point<i32, Logical> {
        self.get_state().location()
    }

    pub fn size(&self) -> Size<i32, Logical> {
        self.get_state().size()
    }

    pub fn z_index(&self) -> u8 {
        if self.is_floating() {
            FLOATING_Z_INDEX
        } else {
            TILING_Z_INDEX
        }
    }

    pub fn get_borders(&self, output: &Output) -> WindowBorder {
        let scale = output.current_scale().fractional_scale();
        let window_loc: Point<i32, Logical> = self.location();
        let (x, y) = (window_loc.x, window_loc.y);
        let window_size: Size<i32, Logical> = self.size();
        let (w, h) = (window_size.w, window_size.h);

        let geometry = Rectangle::from_loc_and_size(window_loc, window_size);
        println!("Drawing border for {:?}", geometry);


        let left = {
            let topleft = (x - 2, y - 2);
            let bottom_right = (x, y + h);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        let top = {
            let topleft = (x, y - 2);
            let bottom_right = (x + w + 2, y);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        let bottom = {
            let topleft = (x - 2, y + h);
            let bottom_right = (x + w + 2, y + h + 2);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        let right = {
            let topleft = (x + w, y);
            let bottom_right = (x + w + 2, y + h + 2);
            Rectangle::from_extemities(topleft, bottom_right)
        };

        let border = WindowBorder {
            left,
            right,
            top,
            bottom
        };

         println!("{:?}", border);
        border

    }
}

impl From<ToplevelSurface> for WindowWrap {
    fn from(toplevel: ToplevelSurface) -> Self {
        let window = Window::new(Kind::Xdg(toplevel));
        window.user_data().insert_if_missing(WindowState::new);

        WindowWrap { inner: window }
    }
}

impl From<Window> for WindowWrap {
    fn from(window: Window) -> Self {
        WindowWrap { inner: window }
    }
}
