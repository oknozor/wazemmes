use crate::backend::drawing::{FLOATING_Z_INDEX, TILING_Z_INDEX};
use crate::backend::xwayland::X11State;
use crate::shell::drawable::{Border, Borders};
use crate::shell::node;
use smithay::desktop::{Kind, Space, Window};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::compositor;
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgToplevelSurfaceRoleAttributes};
use std::cell::RefCell;
use std::fmt::Debug;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct WindowState {
    id: RefCell<u32>,
    floating: RefCell<bool>,
    configured: RefCell<bool>,
    initial_size: RefCell<Size<i32, Logical>>,
    size: RefCell<Size<i32, Logical>>,
    loc: RefCell<Point<i32, Logical>>,
    borders: RefCell<Borders>,
}

impl WindowState {
    fn new() -> Self {
        WindowState {
            id: RefCell::new(node::id::next()),
            floating: RefCell::new(false),
            configured: RefCell::new(false),
            initial_size: RefCell::new(Default::default()),
            size: RefCell::new(Default::default()),
            loc: RefCell::new(Default::default()),
            borders: RefCell::new(Borders::default()),
        }
    }

    pub fn id(&self) -> u32 {
        *self.id.borrow()
    }

    pub fn is_floating(&self) -> bool {
        *self.floating.borrow()
    }

    pub fn configured(&self) -> bool {
        *self.configured.borrow()
    }

    pub fn set_configured(&self) {
        self.configured.replace(true);
    }

    pub fn initial_size(&self) -> Size<i32, Logical> {
        *self.initial_size.borrow()
    }

    pub fn set_initial_geometry(&self, size: Size<i32, Logical>) {
        self.initial_size.replace(size);
    }

    pub fn toggle_floating(&self) {
        let current = *self.floating.borrow();
        self.floating.replace(!current);
    }

    pub fn borders(&self) -> Borders {
        self.borders.borrow().clone()
    }
}

#[derive(Debug, Clone)]
pub struct WindowWrap {
    inner: Window,
}

#[derive(Debug)]
pub struct XdgTopLevelAttributes {
    pub app_id: Option<String>,
    pub title: Option<String>,
}

impl WindowWrap {
    pub fn update_floating(&self, output_geometry: Rectangle<i32, Logical>) -> bool {
        let (size, location) = if self.get_state().configured() {
            let initial_size = self.get_state().initial_size();
            let size = initial_size;
            let location = self.center(output_geometry.size);
            (Some(size), location)
        } else {
            (None, (0, 0).into())
        };

        self.update_loc_and_size(size, location)
    }

    pub fn set_fullscreen(&self, geometry: Rectangle<i32, Logical>) {
        self.update_loc_and_size(Some(geometry.size), geometry.loc);
    }

    pub fn xdg_surface_attributes(&self) -> XdgTopLevelAttributes {
        compositor::with_states(&self.wl_surface(), |states| {
            let guard = states
                .data_map
                .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
                .unwrap()
                .lock()
                .unwrap();

            XdgTopLevelAttributes {
                app_id: guard.app_id.clone(),
                title: guard.title.clone(),
            }
        })
    }

    pub fn get_state(&self) -> &WindowState {
        self.inner.user_data().get::<WindowState>().unwrap()
    }

    pub fn id(&self) -> u32 {
        *self
            .inner
            .user_data()
            .get::<WindowState>()
            .unwrap()
            .id
            .borrow()
    }

    pub fn wl_id(&self) -> u32 {
        self.inner.toplevel().wl_surface().id().protocol_id()
    }

    pub fn location(&self) -> Point<i32, Logical> {
        *self.get_state().loc.borrow()
    }

    pub fn inner(&self) -> &Window {
        &self.inner
    }

    pub fn toplevel(&self) -> Option<&ToplevelSurface> {
        match self.inner.toplevel() {
            Kind::Xdg(toplevel) => Some(toplevel),
            Kind::X11(_xsurface) => None,
        }
    }

    pub fn wl_surface(&self) -> WlSurface {
        match self.inner.toplevel() {
            Kind::Xdg(toplevel) => toplevel.wl_surface().clone(),
            Kind::X11(xsurface) => xsurface.surface.clone(),
        }
    }

    pub fn map(&self, space: &mut Space, x11_state: Option<&mut X11State>, activate: bool) {
        match self.inner.toplevel() {
            Kind::Xdg(toplevel) => {
                toplevel.with_pending_state(|state| {
                    state.size = Some(self.size());
                });

                toplevel.send_configure();
            }

            Kind::X11(x11surface) => {
                let state = x11_state.unwrap();
                let id = x11surface.surface.id().protocol_id();
                state.send_configure(id, Some(self.size())).expect("X11 Error");
            }
        }

        space.map_window(&self.inner, self.loc(), self.z_index(), activate);
    }

    pub fn update_loc_and_size<S, P>(&self, size: Option<S>, location: P) -> bool
    where
        S: Into<Size<i32, Logical>> + Debug,
        P: Into<Point<i32, Logical>> + Debug,
    {
        let state = self.get_state();
        let new_location = location.into();

        let loc_changed = if *state.loc.borrow() != new_location {
            state.loc.replace(new_location);
            true
        } else {
            false
        };

        let size_changed = if let Some(new_size) = size {
            let new_size = new_size.into();
            if *state.size.borrow() != new_size {
                state.size.replace(new_size);
                true
            } else {
                false
            }
        } else {
            false
        };

        if loc_changed || size_changed {
            self.get_state().borders.replace(self.make_borders());
            true
        } else {
            false
        }
    }

    pub fn send_close(&self, x11_state: Option<&mut X11State>) {
        match self.inner.toplevel() {
            Kind::Xdg(toplevel) => toplevel.send_close(),
            Kind::X11(_x11surface) => x11_state.unwrap().send_close(self.wl_id()).expect("X11 Error"),
        }
    }

    pub fn toggle_floating(&self) {
        self.get_state().toggle_floating();
    }

    pub fn is_floating(&self) -> bool {
        self.get_state().is_floating()
    }

    pub fn z_index(&self) -> u8 {
        if self.is_floating() {
            FLOATING_Z_INDEX
        } else {
            TILING_Z_INDEX
        }
    }

    pub fn center(&self, output_size: Size<i32, Logical>) -> Point<i32, Logical> {
        let center_y = output_size.h / 2;
        let center_x = output_size.w / 2;
        let window_geometry = self.inner.geometry();
        let window_center_y = window_geometry.size.h / 2;
        let window_center_x = window_geometry.size.w / 2;
        let x = center_x - window_center_x;
        let y = center_y - window_center_y;
        Point::from((x, y))
    }

    pub fn size(&self) -> Size<i32, Logical> {
        *self.get_state().size.borrow()
    }

    pub fn loc(&self) -> Point<i32, Logical> {
        *self.get_state().loc.borrow()
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

impl WindowWrap {
    pub fn from_x11_window(window: Window) -> WindowWrap {
        window.user_data().insert_if_missing(WindowState::new);
        WindowWrap { inner: window }
    }
}
