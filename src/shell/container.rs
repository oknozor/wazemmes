use crate::shell::tree::ContainerRef;
use smithay::desktop::{Space};
use smithay::utils::Size;
use smithay::wayland::output::Output;
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use std::cell::RefCell;
use std::rc::Rc;
use std::slice::Iter;
use crate::shell::window::WindowWarp;

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
    pub windows: Vec<WindowWarp>,
    pub layout: ContainerLayout,
}

#[derive(Debug)]
pub enum ContainerState {
    Empty,
    HasChildrenOnly,
    HasWindows,
}

#[derive(Debug, Copy, Clone)]
pub enum ContainerLayout {
    Vertical,
    Horizontal,
}

impl Container {
    pub fn state(&self) -> ContainerState {
        if self.has_surface() {
            ContainerState::HasWindows
        } else if self.has_child() {
            ContainerState::HasChildrenOnly
        } else {
            ContainerState::Empty
        }
    }

    fn has_surface(&self) -> bool {
        !self.windows.is_empty()
    }

    fn has_child(&self) -> bool {
        !self.childs.is_empty()
    }

    pub fn get_focused_window(&self) -> Option<(usize, &'_ WindowWarp)> {
        self
            .windows
            .iter()
            .enumerate()
            .find(|(_, surface)| {
                surface
                    .get_toplevel()
                    .current_state()
                    .states
                    .contains(xdg_toplevel::State::Activated)
            })
    }

    pub fn push_window(&mut self, surface: ToplevelSurface, space: &mut Space) {
        println!("Creating new window");
        let window = WindowWarp::from(surface);
        self.windows.push(window);
        self.redraw(space);
    }

    pub fn create_child(&mut self, layout: ContainerLayout, parent: ContainerRef) -> ContainerRef {
        if self.windows.len() <= 1 {
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
                windows: vec![],
                layout,
            };

            let idx = self.get_focused_window()
                .map(|(idx, _)| idx);

            if let Some(idx) = idx {
                let surface = self.windows.remove(idx);
                child.windows.push(surface);
            }

            let child = Rc::new(RefCell::new(child));
            self.childs.push(child.clone());
            println!("Created child container {} with size = {:?} and loc= {:?} in container [{}]", id::get(), (width, height), (x, y), self.id);
            child
        }
    }

    pub fn close_window(&mut self) {
        let idx = self
            .get_focused_window()
            .map(|(idx, window)| {
                window.send_close();
                idx
            });

        if let Some(idx) = idx {
            println!("surface removed");
            let _surface = self.windows.remove(idx);
        }
    }

    pub fn redraw(&self, space: &mut Space) {
        println!("Redraw container {}", self.id);
        let surface_len = self.windows.len();
        let child_len = self.childs.len();
        let len = surface_len + child_len;
        let len = if len == 0 { 1 } else { len };

        println!("Container size: {surface_len} surfaces and {child_len} childs");
        let window_size = match self.layout {
            ContainerLayout::Vertical => (self.width, self.height / len as i32),
            ContainerLayout::Horizontal => (self.width / len as i32, self.height),
        };

        let mut location = (self.x, self.y);

        for (idx, window) in self.windows.iter().enumerate() {
            println!("Configuring surface in container");
            window.get_toplevel().with_pending_state(|state| state.size = Some(Size::from(window_size)));

            if idx > 0 {
                match self.layout {
                    ContainerLayout::Vertical => location = (location.0, location.1 + window_size.1),
                    ContainerLayout::Horizontal => location = (location.0 + window_size.0, location.1),
                }
            };
            let surfaces_nth = self.windows.len() - 1;
            let activate = idx == surfaces_nth;

            window.get_toplevel().with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
                state.size = Some(Size::from(window_size))
            });

            window.get_toplevel().send_configure();
            space.map_window(window.get(), location, None, activate);

            if let Some(parent) = &self.parent {
                let parent = parent.borrow_mut();
                parent.redraw(space);
            }
        }
    }

    pub fn flatten_window(&self) -> Vec<WindowWarp> {
        let mut windows: Vec<WindowWarp> = self.windows
            .iter()
            .cloned()
            .collect();

        for child in &self.childs {
            let child = child.borrow();
            windows.extend(child.flatten_window())
        }

        windows
    }
}
