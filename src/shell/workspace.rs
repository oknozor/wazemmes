use crate::config::CONFIG;
use crate::shell::container::{Container, ContainerLayout, ContainerRef};
use crate::shell::node;
use crate::shell::node::Node;
use crate::shell::nodemap::NodeMap;
use crate::shell::window::WindowWrap;
use slog_scope::{debug, warn};
use smithay::desktop::Space;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Physical, Rectangle};
use smithay::wayland::output::Output;
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct WorkspaceRef {
    inner: Rc<RefCell<Workspace>>,
}

impl From<Workspace> for WorkspaceRef {
    fn from(workspace: Workspace) -> Self {
        WorkspaceRef {
            inner: Rc::new(RefCell::new(workspace)),
        }
    }
}

impl WorkspaceRef {
    pub fn new(output: Output, space: &Space) -> Self {
        let geometry = space.output_geometry(&output).unwrap();
        Self {
            inner: Rc::new(RefCell::new(Workspace::new(&output, geometry))),
        }
    }

    pub fn get_mut(&self) -> RefMut<'_, Workspace> {
        self.inner.borrow_mut()
    }

    pub fn get(&self) -> Ref<'_, Workspace> {
        self.inner.borrow()
    }
}

#[derive(Debug)]
pub struct Workspace {
    pub output: Output,
    pub fullscreen_layer: Option<Node>,
    root: ContainerRef,
    focus: ContainerRef,
}

impl Workspace {
    pub fn new(output: &Output, geometry: Rectangle<i32, Logical>) -> Workspace {
        let gaps = CONFIG.gaps as i32;

        let root = Container {
            id: node::id::get(),
            location: (geometry.loc.x + gaps, geometry.loc.y + gaps).into(),
            size: (geometry.size.w - 2 * gaps, geometry.size.h - 2 * gaps).into(),
            output: output.clone(),
            parent: None,
            nodes: NodeMap::default(),
            layout: ContainerLayout::Horizontal,
        };

        let root = ContainerRef::new(root);
        let focus = root.clone();

        Self {
            output: output.clone(),
            root,
            focus,
            fullscreen_layer: None,
        }
    }

    pub fn redraw(&self, space: &mut Space, dh: &DisplayHandle) {
        self.unmap_all(space);

        if let Some(layer) = &self.fullscreen_layer {
            let geometry = space.output_geometry(&self.output).expect("Geometry");
            match layer {
                Node::Container(container) => {
                    let mut container = container.get_mut();
                    container.size = geometry.size;
                    container.location = geometry.loc;
                    container.redraw(space);
                }
                Node::Window(window) => {
                    window.toggle_fullscreen(space, geometry);
                }
            }
        } else {
            let mut root = self.root.get_mut();
            root.redraw(space);
        }

        space.refresh(dh)
    }

    pub fn root(&self) -> ContainerRef {
        self.root.clone()
    }

    pub fn get_focus(&self) -> (ContainerRef, Option<WindowWrap>) {
        // FIXME: panic here some time
        let window = {
            let c = self.focus.get();
            c.get_focused_window()
        };

        (self.focus.clone(), window)
    }

    pub fn create_container(&mut self, layout: ContainerLayout) -> ContainerRef {
        let child = {
            let (container, _) = self.get_focus();
            let parent = container.clone();
            let mut current = container.get_mut();
            current.create_child(layout, parent)
        };

        self.focus = child.clone();
        child
    }

    pub fn pop_container(&mut self) {
        let current = self.get_focus();
        let current = current.0.get();
        let id = current.id;
        if let Some(parent) = &current.parent {
            self.focus = parent.clone();
            let mut parent = parent.get_mut();
            parent.nodes.remove(&id);
        }
    }

    pub fn set_container_focused(&mut self, id: u32) {
        if let Some(container) = self.find_container_by_id(&id) {
            self.focus = container;
        } else {
            warn!("Tried to set container focus but container with id [{id}] was not found")
        }
    }

    pub fn flatten_window(&self) -> Vec<WindowWrap> {
        let root = self.root.get();
        let mut windows: Vec<WindowWrap> = root.nodes.iter_windows().cloned().collect();

        for child in root.nodes.iter_containers() {
            let window = child.get().flatten_window();
            windows.extend_from_slice(window.as_slice());
        }

        windows
    }

    pub fn unmap_all(&self, space: &mut Space) {
        for window in self.flatten_window() {
            space.unmap_window(window.get());
        }
    }

    pub fn map_all(&self, space: &mut Space, dh: &DisplayHandle) {
        let root = self.root();
        let mut root = root.get_mut();
        debug!("Redraw root container from `Workspace::map_all`");
        root.redraw(space);
        space.refresh(dh);
    }

    pub fn find_container_by_id(&self, id: &u32) -> Option<ContainerRef> {
        if &self.root.get().id == id {
            Some(self.root.clone())
        } else {
            self.root.find_container_by_id(id)
        }
    }

    pub fn reset_gaps(&self, space: &Space) {
        let gaps = CONFIG.gaps as i32;
        let geometry = space
            .output_geometry(&self.output)
            .expect("Output should have a geometry");
        let mut root = self.root.get_mut();
        root.location = (geometry.loc.x + gaps, geometry.loc.y + gaps).into();
        root.size = (geometry.size.w - 2 * gaps, geometry.size.h - 2 * gaps).into();
    }

    pub fn get_output_geometry_f64(&self, space: &Space) -> Option<Rectangle<f64, Physical>> {
        space.output_geometry(&self.output).map(|geometry| {
            let scale = self.output.current_scale().fractional_scale();
            geometry.to_f64().to_physical_precise_up(scale)
        })
    }
}
