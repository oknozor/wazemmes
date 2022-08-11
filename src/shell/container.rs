use crate::shell::window::WindowWarp;

use smithay::desktop::Space;
use smithay::wayland::output::Output;
use smithay::wayland::shell::xdg::ToplevelSurface;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct ContainerRef {
    inner: Rc<RefCell<Container>>,
}

impl ContainerRef {
    pub fn new(container: Container) -> Self {
        ContainerRef {
            inner: Rc::new(RefCell::new(container)),
        }
    }

    pub fn get(&self) -> Ref<'_, Container> {
        self.inner.borrow()
    }

    pub fn get_mut(&self) -> RefMut<'_, Container> {
        self.inner.borrow_mut()
    }

    pub fn container_having_window(&self, id: u32) -> Option<ContainerRef> {
        let this = self.get();

        if this.windows.contains_key(&id) {
            Some(self.clone())
        } else {
            this.childs
                .iter()
                .find_map(|c| c.container_having_window(id))
        }
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
    pub windows: HashMap<u32, WindowWarp>,
    pub layout: ContainerLayout,
    pub focus: Option<u32>,
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

    pub fn get_focused_window(&self) -> Option<(u32, &'_ WindowWarp)> {
        if let Some(focus) = self.focus {
            let window = self.windows.get(&focus);
            window.map(|window| (focus, window))
        } else {
            None
        }
    }

    // Push a window to the tree and return its index
    pub fn push_window(&mut self, surface: ToplevelSurface) -> u32 {
        let window = WindowWarp::new(surface);
        let id = window.id();
        self.windows.insert(id, window);
        id
    }

    pub fn create_child(&mut self, layout: ContainerLayout, parent: ContainerRef) -> ContainerRef {
        if self.windows.len() <= 1 {
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
                windows: HashMap::new(),
                layout,
                focus: None,
            };

            let id = self.get_focused_window().map(|(id, _)| id);

            if let Some(id) = id {
                let window = self.windows.remove(&id);
                if let Some(window) = window {
                    child.windows.insert(id, window);
                }
            }

            let child = ContainerRef::new(child);
            self.childs.push(child.clone());
            child
        }
    }

    pub fn close_window(&mut self) {
        let idx = self.get_focused_window().map(|(idx, window)| {
            window.send_close();
            idx
        });

        if let Some(idx) = idx {
            let _surface = self.windows.remove(&idx);

            if self.windows.is_empty() {
                self.focus = None
            } else if self.windows.get(&(idx - 1)).is_some() {
                self.focus = Some(idx - 1)
            };
        }
    }

    // Fully redraw a container, its window an children containers
    // Call this on the root of the tree to refresh a workspace
    pub fn redraw(&self, space: &mut Space) {
        let surface_len = self.windows.len();
        let child_len = self.childs.len();
        let len = surface_len + child_len;
        let len = if len == 0 { 1 } else { len };

        let size = match self.layout {
            ContainerLayout::Vertical => (self.width, self.height / len as i32),
            ContainerLayout::Horizontal => (self.width / len as i32, self.height),
        };

        let mut location = (self.x, self.y);

        for (idx, (id, window)) in self.windows.iter().enumerate() {
            if idx > 0 {
                match self.layout {
                    ContainerLayout::Vertical => location = (location.0, location.1 + size.1),
                    ContainerLayout::Horizontal => location = (location.0 + size.0, location.1),
                }
            };

            let activate = Some(*id) == self.focus;
            window.configure(space, size, activate);
            space.map_window(window.get(), location, None, activate);

            for child in &self.childs {
                let child = child.get();
                child.redraw(space);
            }
        }
    }

    pub fn flatten_window(&self) -> Vec<WindowWarp> {
        let mut windows: Vec<WindowWarp> = self.windows.values().cloned().collect();

        for child in &self.childs {
            let child = child.get();
            windows.extend(child.flatten_window())
        }

        windows
    }

    pub fn flatten_containers(&self) -> Vec<ContainerRef> {
        let mut children: Vec<ContainerRef> = self.childs.clone();

        for child in &self.childs {
            let child = child.get();
            children.extend(child.flatten_containers())
        }

        children
    }

    pub fn set_focus(&mut self, window_idx: u32) {
        if self.windows.get(&window_idx).is_some() {
            self.focus = Some(window_idx)
        }
    }
}

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
