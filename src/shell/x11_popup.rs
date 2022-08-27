use crate::backend::drawing::POP_UP_Z_INDEX;
use crate::shell::node;
use slog_scope::debug;
use smithay::desktop::{Space, Window};
use smithay::utils::{IsAlive, Logical, Point};
use std::cell::RefCell;

pub struct X11PopupState {
    pub id: u32,
    pub loc: RefCell<Point<i32, Logical>>,
    pub needs_initial_render: RefCell<bool>,
}

#[derive(Debug, Clone)]
pub struct X11Popup(Window);

impl X11Popup {
    pub fn new(window: Window, loc: Point<i32, Logical>) -> Self {
        window.user_data().insert_if_missing(|| X11PopupState {
            id: node::id::next(),
            loc: RefCell::new(loc),
            needs_initial_render: RefCell::new(true),
        });

        X11Popup(window)
    }

    pub fn shift_location(&self, translate: Point<i32, Logical>) {
        debug!("Shifting xpopup initial location");
        let loc = &self.0.user_data().get::<X11PopupState>().unwrap().loc;

        let current_loc = *loc.borrow();

        loc.replace((current_loc.x + translate.x, current_loc.y + translate.y).into());
    }

    pub fn map(&self, space: &mut Space) {
        let state = self.0.user_data().get::<X11PopupState>().unwrap();

        state.needs_initial_render.replace(false);
        let location = *state.loc.borrow();
        space.map_window(&self.0, location, POP_UP_Z_INDEX, true)
    }

    pub fn id(&self) -> u32 {
        self.0.user_data().get::<X11PopupState>().unwrap().id
    }

    pub fn needs_initial_render(&self) -> bool {
        *self
            .0
            .user_data()
            .get::<X11PopupState>()
            .unwrap()
            .needs_initial_render
            .borrow()
    }

    pub fn alive(&self) -> bool {
        self.0.alive()
    }
}
