use crate::shell::window::WindowWarp;

use crate::shell::node;
use crate::shell::node::Node;
use slog_scope::debug;
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

        if this.childs.contains_key(&id) {
            Some(self.clone())
        } else {
            this.iter_containers()
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
    pub childs: HashMap<u32, Node>,
    pub layout: ContainerLayout,
    pub focus: Option<u32>,
}

#[derive(Debug)]
pub enum ContainerState {
    Empty,
    HasContainersOnly,
    HasWindows,
}

#[derive(Debug, Copy, Clone)]
pub enum ContainerLayout {
    Vertical,
    Horizontal,
}

impl Container {
    pub fn state(&self) -> ContainerState {
        if self.has_windows() {
            ContainerState::HasWindows
        } else if self.has_child_containers() {
            ContainerState::HasContainersOnly
        } else {
            ContainerState::Empty
        }
    }

    fn has_windows(&self) -> bool {
        self.iter_windows().count() > 0
    }

    fn has_child_containers(&self) -> bool {
        self.childs.values().any(|c| c.is_container())
    }

    pub fn get_focused_window(&self) -> Option<(u32, &'_ WindowWarp)> {
        if let Some(focus) = self.focus {
            let window = self.childs.get(&focus);
            window.map(|window| (focus, window.try_into().unwrap()))
        } else {
            None
        }
    }

    // Push a window to the tree and return its index
    pub fn push_window(&mut self, surface: ToplevelSurface) -> u32 {
        let window = WindowWarp::new(surface);
        let id = window.id();
        self.childs.insert(id, Node::Window(window));
        id
    }

    pub fn create_child(&mut self, layout: ContainerLayout, parent: ContainerRef) -> ContainerRef {
        if self.iter_windows().count() <= 1 {
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
                id: node::id::next(),
                x,
                y,
                width,
                height,
                output: self.output.clone(),
                parent: Some(parent),
                childs: HashMap::new(),
                layout,
                focus: None,
            };

            let id = self.get_focused_window().map(|(id, _)| id);

            if let Some(id) = id {
                let window = self.childs.remove(&id);
                if let Some(window) = window {
                    child.childs.insert(id, window);
                }
            }

            let id = child.id;
            let child = ContainerRef::new(child);
            self.childs.insert(id, Node::Container(child.clone()));
            child
        }
    }

    pub fn close_window(&mut self) {
        let idx = self.get_focused_window().map(|(idx, window)| {
            window.send_close();
            idx
        });

        if let Some(idx) = idx {
            let _surface = self.childs.remove(&idx);

            if self.iter_windows().count() == 0 {
                self.focus = None
            } else if self.childs.get(&(idx - 1)).is_some() {
                // Fixme: previous window
                // self.focus = Some(idx - 1)
            };
        }
    }

    // Fully redraw a container, its window an children containers
    // Call this on the root of the tree to refresh a workspace
    pub fn redraw(&mut self, space: &mut Space) {
        let len = self.childs.len();
        let len = if len == 0 { 1 } else { len };

        let (width, height) = match self.layout {
            ContainerLayout::Vertical => (self.width, self.height / len as i32),
            ContainerLayout::Horizontal => (self.width / len as i32, self.height),
        };

        let (mut x, mut y) = (self.x, self.y);

        self.reparent_orphans();

        for (idx, (id, node)) in self.childs.iter().enumerate() {
            if idx > 0 {
                match self.layout {
                    ContainerLayout::Vertical => y += height,
                    ContainerLayout::Horizontal => x += width,
                }
            };

            match node {
                Node::Container(container) => {
                    let mut child = container.get_mut();
                    child.x = x;
                    child.y = y;
                    child.width = width;
                    child.height = height;
                    child.redraw(space);
                }
                Node::Window(window) => {
                    let activate = Some(*id) == self.focus;
                    debug!(
                        "Placing window (x={}, y={}, w={}, h={})",
                        x, y, width, height
                    );
                    window.configure(space, (width, height), activate);
                    space.map_window(window.get(), (x, y), None, activate);
                }
            }
        }
    }

    fn reparent_orphans(&mut self) {
        let mut orphans = vec![];

        for child in self.iter_containers() {
            let mut child = child.get_mut();
            if child.iter_windows().count() == 0 {
                let children = child.drain_containers();
                orphans.extend_from_slice(children.as_slice());
            }
        }

        self.childs.extend(orphans);
    }

    pub fn flatten_window(&self) -> Vec<WindowWarp> {
        let mut windows: Vec<WindowWarp> = self.iter_windows().cloned().collect();

        for child in self.iter_containers() {
            let child = child.get();
            windows.extend(child.flatten_window())
        }

        windows
    }

    pub fn set_focus(&mut self, window_idx: u32) {
        if self.childs.get(&window_idx).is_some() {
            self.focus = Some(window_idx)
        }
    }

    pub fn iter_windows(&self) -> impl Iterator<Item = &WindowWarp> {
        self.childs.values().filter_map(|node| match node {
            Node::Container(_) => None,
            Node::Window(w) => Some(w),
        })
    }

    pub fn iter_containers(&self) -> impl Iterator<Item = &ContainerRef> {
        self.childs.values().filter_map(|node| match node {
            Node::Container(c) => Some(c),
            Node::Window(_) => None,
        })
    }

    fn drain_containers(&mut self) -> Vec<(u32, Node)> {
        self.childs.drain_filter(|_k, v| v.is_container()).collect()
    }
}
