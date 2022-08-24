use crate::backend::drawing::{FLOATING_Z_INDEX, TILING_Z_INDEX};
use crate::shell::node;
use slog_scope::debug;
use smithay::desktop::{Kind, Space, Window};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::compositor;
use smithay::wayland::output::Output;
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgToplevelSurfaceRoleAttributes};
use std::cell::RefCell;
use std::sync::Mutex;

#[derive(Debug, Clone)]
pub struct WindowState {
    id: RefCell<u32>,
    floating: RefCell<bool>,
    configured: RefCell<bool>,
    initial_size: RefCell<Size<i32, Logical>>,
}

impl WindowState {
    fn new() -> Self {
        Self {
            id: RefCell::new(node::id::next()),
            floating: RefCell::new(false),
            configured: RefCell::new(false),
            initial_size: RefCell::new(Default::default()),
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

    fn toggle_floating(&self) {
        debug!(
            "(WindowState) - Floating toggle for window[{}]",
            *self.id.borrow()
        );
        let current = *self.floating.borrow();
        self.floating.replace(!current);
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
    pub fn update_floating(&self, space: &mut Space, output: &Output, activate: bool) {
        let (size, location) = if self.get_state().configured() {
            let output_geometry = space.output_geometry(output).unwrap();
            let initial_size = self.get_state().initial_size();
            let size = initial_size;
            let location = self.center(output_geometry.size);
            (Some(size), location)
        } else {
            (None, (0, 0).into())
        };

        self.configure(space, size, location, activate);
    }

    pub fn toggle_fullscreen(&self, space: &mut Space, geometry: Rectangle<i32, Logical>) {
        self.configure(space, Some(geometry.size), geometry.loc, true);
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
        self.inner.user_data().get::<WindowState>().unwrap().id()
    }

    pub fn get(&self) -> &Window {
        &self.inner
    }

    pub fn toplevel(&self) -> Option<&ToplevelSurface> {
        match self.inner.toplevel() {
            Kind::Xdg(toplevel) => Some(toplevel),
            // TODO: What to do here?
            Kind::X11(_xsurface) => None,
        }
    }

    pub fn wl_surface(&self) -> WlSurface {
        match self.inner.toplevel() {
            Kind::Xdg(toplevel) => toplevel.wl_surface().clone(),
            Kind::X11(xsurface) => xsurface.surface.clone(),
        }
    }

    pub fn configure<S, P>(&self, space: &mut Space, size: Option<S>, location: P, activate: bool)
    where
        S: Into<Size<i32, Logical>>,
        P: Into<Point<i32, Logical>>,
    {
        let toplevel = self.toplevel();

        // TODO: What about x11 here ?
        if let (Some(size), Some(toplevel)) = (size, toplevel) {
            toplevel.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
                state.size = Some(size.into())
            });

            toplevel.send_configure();
        }

        space.map_window(&self.inner, location, self.z_index(), activate);
    }

    pub fn send_close(&self) {
        if let Some(toplevel) = self.toplevel() {
            toplevel.send_close()
        }
    }

    pub fn toggle_floating(&self) {
        debug!("(WindowWrap) - Floating toggle  for window[{}]", self.id());
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
