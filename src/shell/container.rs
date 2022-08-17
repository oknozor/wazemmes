use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use smithay::desktop::Space;
use smithay::wayland::output::Output;
use smithay::wayland::shell::xdg::ToplevelSurface;

use crate::config::CONFIG;
use crate::shell::node;
use crate::shell::node::Node;
use crate::shell::nodemap::NodeMap;
use crate::shell::window::WindowWarp;

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

        if this.nodes.contains(&id) {
            Some(self.clone())
        } else {
            this.nodes
                .iter_containers()
                .find_map(|c| c.container_having_window(id))
        }
    }

    pub fn find_container_by_id(&self, id: &u32) -> Option<ContainerRef> {
        let this = self.get();
        if &this.id == id {
            Some(self.clone())
        } else {
            this.nodes
                .items
                .get(id)
                .and_then(|node| node.try_into().ok())
        }
        .or_else(|| {
            this.nodes
                .iter_containers()
                .find_map(|c| c.find_container_by_id(id))
        })
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
    pub nodes: NodeMap,
    pub layout: ContainerLayout,
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
        } else if self.has_container() {
            ContainerState::HasContainersOnly
        } else {
            ContainerState::Empty
        }
    }

    fn has_windows(&self) -> bool {
        self.nodes.has_window()
    }

    pub fn has_container(&self) -> bool {
        self.nodes.has_container()
    }

    pub fn get_focused_window(&self) -> Option<(u32, &'_ WindowWarp)> {
        if let Some(focus) = self.nodes.get_focused() {
            let window = self.nodes.get(focus);
            window.map(|window| (*focus, window.try_into().unwrap()))
        } else {
            None
        }
    }

    pub fn get_focused_window_mut(&mut self) -> Option<(u32, &'_ mut WindowWarp)> {
        let focused = self.nodes.get_focused().cloned();
        if let Some(focus) = focused {
            let window = self.nodes.get_mut(&focus);
            window.map(|window| (focus, window.try_into().unwrap()))
        } else {
            None
        }
    }

    // Push a window to the tree and update the focus
    pub fn push_window(&mut self, surface: ToplevelSurface) {
        let window = WindowWarp::from(surface);
        let id = window.id();
        self.nodes.insert(id, Node::Window(window));
    }

    pub fn create_child(&mut self, layout: ContainerLayout, parent: ContainerRef) -> ContainerRef {
        if self.nodes.iter_windows().count() <= 1 {
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
                nodes: NodeMap::default(),
                layout,
            };

            let id = self.get_focused_window().map(|(id, _)| id);

            if let Some(id) = id {
                let window = self.nodes.remove(&id);
                if let Some(window) = window {
                    child.nodes.insert(id, window);
                }
            }

            let id = child.id;
            let child = ContainerRef::new(child);
            self.nodes.insert(id, Node::Container(child.clone()));
            child
        }
    }

    pub fn close_window(&mut self) {
        let idx = self.get_focused_window().map(|(idx, window)| {
            window.send_close();
            idx
        });

        if let Some(id) = idx {
            let _surface = self.nodes.remove(&id);
        }
    }

    // Fully redraw a container, its window an children containers
    // Call this on the root of the tree to refresh a workspace
    pub fn redraw(&mut self, space: &mut Space) {
        if self.nodes.tiled_element_len() == 0 {
            return;
        }

        let len = self.nodes.tiled_element_len();
        let non_zero_length = if len == 0 { 1 } else { len };
        let gaps = CONFIG.gaps as i32;
        let total_gaps = (len - 1) as i32 * gaps;

        let (width, height) = match self.layout {
            ContainerLayout::Vertical => {
                let w = self.width;
                let h = (self.height - total_gaps) / non_zero_length as i32;
                (w, h)
            }
            ContainerLayout::Horizontal => {
                let w = (self.width - total_gaps) / non_zero_length as i32;
                let h = self.height;
                (w, h)
            }
        };

        let (mut x, mut y) = (self.x, self.y);

        self.reparent_orphans();

        for (idx, id, node) in self.nodes.iter_spine() {
            match node {
                Node::Container(container) => {
                    if idx > 0 {
                        match self.layout {
                            ContainerLayout::Vertical => y += height + gaps,
                            ContainerLayout::Horizontal => x += width + gaps,
                        }
                    };

                    let mut child = container.get_mut();
                    child.x = x;
                    child.y = y;
                    child.width = width;
                    child.height = height;
                    child.redraw(space);
                }
                Node::Window(window) if !window.is_floating() => {
                    if idx > 0 {
                        match self.layout {
                            ContainerLayout::Vertical => y += height + gaps,
                            ContainerLayout::Horizontal => x += width + gaps,
                        }
                    };

                    let activate = Some(*id) == self.get_focused_window().map(|(id, _w)| id);
                    window.get_state().set_location((x, y));
                    window.configure(space, (width, height), activate);
                    space.map_window(window.get(), (x, y), None, activate);
                }
                Node::Window(_) => {}
            }
        }
    }

    fn reparent_orphans(&mut self) {
        let mut orphans = vec![];

        for child in self.nodes.iter_containers() {
            let mut child = child.get_mut();
            if child.nodes.iter_windows().count() == 0 {
                let children = child.nodes.drain_containers();
                orphans.extend_from_slice(children.as_slice());
            }
        }

        self.nodes.extend(orphans);
    }

    pub fn flatten_window(&self) -> Vec<WindowWarp> {
        let mut windows: Vec<WindowWarp> = self.nodes.iter_windows().cloned().collect();

        for child in self.nodes.iter_containers() {
            let child = child.get();
            windows.extend(child.flatten_window())
        }

        windows
    }

    pub fn set_focus(&mut self, window_id: u32) {
        if self.nodes.get(&window_id).is_some() {
            self.nodes.set_focus(window_id)
        }
    }
}
