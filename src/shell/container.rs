use std::cell::{Ref, RefCell, RefMut};
use std::num::NonZeroUsize;
use std::rc::Rc;

use smithay::desktop::Space;
use smithay::utils::{Logical, Point, Size};

use smithay::wayland::output::Output;
use smithay::wayland::shell::xdg::ToplevelSurface;

use crate::config::CONFIG;
use crate::shell::node;
use crate::shell::node::Node;

use crate::shell::nodemap::NodeMap;
use crate::shell::window::WindowWrap;

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
    pub location: Point<i32, Logical>,
    pub size: Size<i32, Logical>,
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

    pub fn get_focused_window(&self) -> Option<WindowWrap> {
        if let Some(focus) = self.nodes.get_focused() {
            let window = self.nodes.get(focus);
            window.map(|window| window.try_into().unwrap())
        } else {
            None
        }
    }

    pub fn flatten_window(&self) -> Vec<WindowWrap> {
        let mut windows: Vec<WindowWrap> = self.nodes.iter_windows().cloned().collect();

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

    pub fn get_focused_window_mut(&mut self) -> Option<(u32, &'_ mut WindowWrap)> {
        let focused = self.nodes.get_focused().cloned();
        if let Some(focus) = focused {
            let window = self.nodes.get_mut(&focus);
            window.map(|window| (focus, window.try_into().unwrap()))
        } else {
            None
        }
    }

    // Push a window to the tree and update the focus
    pub fn push_window(&mut self, surface: ToplevelSurface) -> u32 {
        let window = WindowWrap::from(surface);
        let id = window.id();
        self.nodes.insert(id, Node::Window(window));
        id
    }

    pub fn create_child(&mut self, layout: ContainerLayout, parent: ContainerRef) -> ContainerRef {
        if self.nodes.iter_windows().count() <= 1 {
            self.layout = layout;
            parent
        } else {
            let size = match self.layout {
                ContainerLayout::Vertical => (self.size.w, self.size.h / 2),
                ContainerLayout::Horizontal => (self.size.w / 2, self.size.h),
            }
            .into();

            let location = match self.layout {
                ContainerLayout::Vertical => (self.location.x, self.location.y + self.size.h),
                ContainerLayout::Horizontal => (self.location.x + self.size.w, self.location.y),
            }
            .into();

            let mut child = Container {
                id: node::id::next(),
                location,
                size,
                output: self.output.clone(),
                parent: Some(parent),
                nodes: NodeMap::default(),
                layout,
            };

            let id = self.get_focused_window().map(|window| window.id());

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
        let idx = self.get_focused_window().map(|window| {
            window.send_close();
            window.id()
        });

        if let Some(id) = idx {
            let _surface = self.nodes.remove(&id);
        }
    }

    // Fully redraw a container, its window an children containers
    // Call this on the root of the tree to refresh a workspace
    pub fn redraw(&mut self, space: &mut Space) {
        // Ensure dead widow get remove before updating the container
        self.nodes.remove_dead_windows();

        // Don't draw anything if the container is empty
        if self.nodes.spine.is_empty() {
            return;
        }

        // reparent sub children having containers only
        self.reparent_orphans();

        // Initial geometry
        let focused_window_id = self.get_focused_window().map(|window| window.id());
        if let Some(size) = self.get_child_size() {
            // Draw everything
            let mut tiling_index = 0;
            for (id, node) in self.nodes.iter_spine() {
                match node {
                    Node::Container(container) => {
                        let mut child = container.get_mut();
                        child.location = self.get_loc_for_index(tiling_index, size);
                        child.size = size;
                        child.redraw(space);
                        tiling_index += 1;
                    }

                    Node::Window(window) if window.is_floating() => {
                        let activate = Some(*id) == focused_window_id;
                        window.update_floating(space, &self.output, activate);
                    }

                    Node::Window(window) => {
                        let activate = Some(*id) == focused_window_id;
                        let loc = self.get_loc_for_index(tiling_index, size);
                        window.configure(space, Some(size), loc, activate);
                        tiling_index += 1;
                    }
                }
            }
        } else {
            // Draw floating elements only
            for (id, node) in self.nodes.iter_spine() {
                match node {
                    Node::Window(window) if window.is_floating() => {
                        let activate = Some(*id) == focused_window_id;
                        window.update_floating(space, &self.output, activate);
                    }
                    _ => unreachable!("Container should only have floating windows"),
                }
            }
        }
    }

    fn get_child_size(&self) -> Option<Size<i32, Logical>> {
        self.nodes
            .tiled_element_len()
            .map(NonZeroUsize::get)
            .map(|len| {
                if len == 1 {
                    self.size
                } else {
                    let len = len as i32;
                    let gaps = CONFIG.gaps as i32;
                    let total_gaps = gaps * (len - 1);
                    match self.layout {
                        ContainerLayout::Vertical => {
                            let w = self.size.w;
                            let h = (self.size.h - total_gaps) / len;
                            (w, h)
                        }
                        ContainerLayout::Horizontal => {
                            let w = (self.size.w - total_gaps) / len;
                            let h = self.size.h;
                            (w, h)
                        }
                    }
                    .into()
                }
            })
    }

    fn get_loc_for_index(&self, idx: usize, size: Size<i32, Logical>) -> Point<i32, Logical> {
        if idx == 0 {
            self.location
        } else {
            let gaps = CONFIG.gaps as i32;
            let pos = idx as i32;

            match self.layout {
                ContainerLayout::Vertical => {
                    let x = self.location.x;
                    let y = self.location.y + (size.h + gaps) * pos;
                    (x, y)
                }
                ContainerLayout::Horizontal => {
                    let x = self.location.x + (size.w + gaps) * pos;
                    let y = self.location.y;
                    (x, y)
                }
            }
            .into()
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
}
