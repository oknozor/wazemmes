use crate::shell::tree::ContainerRef;
use smithay::desktop::{Kind, Space, Window};
use smithay::utils::Size;
use smithay::wayland::output::Output;
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use std::cell::RefCell;
use std::rc::Rc;

pub mod id {
    use once_cell::sync::Lazy;
    use std::sync::{Arc, Mutex};

    static CONTAINER_ID_COUNTER: Lazy<Arc<Mutex<u32>>> = Lazy::new(|| Arc::new(Mutex::new(0)));

    pub fn get() -> u32 {
        let id = CONTAINER_ID_COUNTER.lock().unwrap();
        *id
    }

    pub fn next() -> u32 {
        let mut id = CONTAINER_ID_COUNTER.lock().unwrap();
        *id += 1;
        *id
    }
}

#[derive(Debug)]
pub struct Container {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub output: Output,
    pub parent: Option<ContainerRef>,
    pub childs: Vec<ContainerRef>,
    pub surfaces: Vec<ToplevelSurface>,
    pub layout: ContainerLayout,
}

#[derive(Debug)]
pub enum ContainerState {
    Empty,
    HasChildsOnly,
    NotEmpty,
}

#[derive(Debug, Copy, Clone)]
pub enum ContainerLayout {
    Vertical,
    Horizontal,
}

impl Container {
    pub fn state(&self) -> ContainerState {
        if self.has_surface() {
            ContainerState::NotEmpty
        } else if self.has_child() {
            ContainerState::HasChildsOnly
        } else {
            ContainerState::Empty
        }
    }

    fn has_surface(&self) -> bool {
        !self.surfaces.is_empty()
    }

    fn has_child(&self) -> bool {
        !self.childs.is_empty()
    }

    pub fn get_focused_surface(&self) -> Option<(usize, &'_ ToplevelSurface)> {
        self
            .surfaces
            .iter()
            .enumerate()
            .find(|(_, surface)| {
                surface
                    .current_state()
                    .states
                    .contains(xdg_toplevel::State::Activated)
            })
    }

    pub fn push_window(&mut self, surface: ToplevelSurface, space: &mut Space) {
        println!("Creating new window");
        self.surfaces.push(surface);
        self.redraw(space);
    }

    pub fn create_child(&mut self, layout: ContainerLayout, parent: ContainerRef) -> ContainerRef {
        if self.surfaces.len() <= 1 {
            println!("Only one surface, changing layout to {layout:?}");
            self.layout = layout;
            parent
        } else {
            let (width, height) = match self.layout {
                ContainerLayout::Vertical => (self.width, self.height / 2),
                ContainerLayout::Horizontal => (self.width / 2, self.height),
            };

            let (x, y) = match self.layout {
                ContainerLayout::Vertical => (self.x, self.y + height),
                ContainerLayout::Horizontal => (self.x + width, self.y),
            };

            let mut child = Container {
                id: id::next(),
                x,
                y,
                width,
                height,
                output: self.output.clone(),
                parent: Some(parent),
                childs: vec![],
                surfaces: vec![],
                layout,
            };

            let idx = self.get_focused_surface()
                .map(|(idx, _)| idx);

            if let Some(idx) = idx {
                let surface = self.surfaces.remove(idx);
                child.surfaces.push(surface);
            }

            let child = Rc::new(RefCell::new(child));
            self.childs.push(child.clone());
            println!("Created child container {} with size = {:?} and loc= {:?} in container [{}]", id::get(), (width, height), (x, y), self.id);
            child
        }
    }

    pub fn close_surface(&mut self) {
        let idx = self
            .get_focused_surface()
            .map(|(idx, surface)| {
                surface.send_close();
                idx
            });

        if let Some(idx) = idx {
            println!("surface removed");
            let _surface = self.surfaces.remove(idx);
        }
    }

    pub fn redraw(&self, space: &mut Space) {
        println!("Redraw container {}", self.id);
        let surface_len = self.surfaces.len();
        let child_len = self.childs.len();
        let len = surface_len + child_len;
        let len = if len == 0 { 1 } else { len };

        println!("Container size: {surface_len} surfaces and {child_len} childs");
        let window_size = match self.layout {
            ContainerLayout::Vertical => (self.width, self.height / len as i32),
            ContainerLayout::Horizontal => (self.width / len as i32, self.height),
        };

        let mut location = (self.x, self.y);

        for (idx, surface) in self.surfaces.iter().enumerate() {
            println!("Configuring surface in container");
            surface.with_pending_state(|state| state.size = Some(Size::from(window_size)));
            let window = Window::new(Kind::Xdg(surface.clone()));

            if idx > 0 {
                match self.layout {
                    ContainerLayout::Vertical => location = (location.0, location.1 + window_size.1),
                    ContainerLayout::Horizontal => location = (location.0 + window_size.0, location.1),
                }
            };
            let surfaces_nth = self.surfaces.len() - 1;
            let activate = idx == surfaces_nth;

            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
                state.size = Some(Size::from(window_size))
            });

            surface.send_configure();
            space.map_window(&window, location, None, activate);

            if let Some(parent) = &self.parent {
                let parent = parent.borrow_mut();
                parent.redraw(space);
            }
        }
    }
}
