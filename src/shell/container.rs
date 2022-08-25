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

#[derive(Debug, Eq, PartialEq)]
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

    pub fn get_focus(&self) -> Option<&Node> {
        self.nodes.get_focused()
    }

    pub fn toggle_fullscreen(&mut self, space: &mut Space) {
        let gaps = CONFIG.gaps as i32;
        let geometry = space
            .output_geometry(&self.output)
            .expect("No output geometry");

        self.location = (geometry.loc.x + gaps, geometry.loc.y + gaps).into();
        self.size = (geometry.size.w - 2 * gaps, geometry.size.h - 2 * gaps).into();
        self.redraw(space);
    }

    pub fn get_focused_window(&self) -> Option<WindowWrap> {
        self.nodes.get_focused().and_then(|node| match node {
            Node::Container(_) => None,
            Node::Window(window) => Some(window.clone()),
        })
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

    // Push a window to the tree and update the focus
    pub fn push_window(&mut self, surface: ToplevelSurface) -> u32 {
        let window = Node::Window(WindowWrap::from(surface));
        match self.get_focused_window() {
            None => self.nodes.push(window),
            Some(focus) => self
                .nodes
                .insert_after(focus.id(), window)
                .expect("Should insert window"),
        }
    }

    pub fn insert_window_after(&mut self, target_id: u32, window: WindowWrap) {
        let id = window.id();
        self.nodes.insert_after(target_id, Node::Window(window));
        self.nodes.set_focus(id);
    }

    pub fn insert_window_before(&mut self, target_id: u32, window: WindowWrap) {
        let id = window.id();
        self.nodes.insert_before(target_id, Node::Window(window));
        self.nodes.set_focus(id);
    }

    pub fn create_child(&mut self, layout: ContainerLayout, parent: ContainerRef) -> ContainerRef {
        if self.nodes.spine.len() <= 1 {
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

            let child = Container {
                id: node::id::next(),
                location,
                size,
                output: self.output.clone(),
                parent: Some(parent),
                nodes: NodeMap::default(),
                layout,
            };

            let child_ref = ContainerRef::new(child);
            if let Some(focus) = self.get_focused_window() {
                let focus_id = focus.id();
                self.nodes
                    .insert_after(focus_id, Node::Container(child_ref.clone()));
                let focus = self
                    .nodes
                    .remove(&focus_id)
                    .expect("Focused window node should exists");
                child_ref.get_mut().nodes.push(focus);
            } else {
                self.nodes.push(Node::Container(child_ref.clone()));
            }

            child_ref
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

    pub fn reparent_orphans(&mut self) {
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
