use crate::config::CONFIG;
use crate::shell::container::{Container, ContainerLayout, ContainerRef};
use crate::shell::node;
use crate::shell::nodemap::NodeMap;
use crate::shell::window::WindowWrap;
use slog_scope::warn;
use smithay::backend::renderer::gles2::Gles2Renderer;
use smithay::desktop::Space;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Rectangle};
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
    root: ContainerRef,
    focus: ContainerRef,
}

impl Workspace {
    pub fn new(output: &Output, geometry: Rectangle<i32, Logical>) -> Workspace {
        let gaps = CONFIG.gaps as i32;
        let root = Container {
            id: node::id::get(),
            x: geometry.loc.x + gaps,
            y: geometry.loc.y + gaps,
            width: geometry.size.w - (2 * gaps),
            height: geometry.size.h - (2 * gaps),
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
        }
    }

    pub fn root(&self) -> ContainerRef {
        self.root.clone()
    }

    pub fn get_focus(&self) -> (ContainerRef, Option<WindowWrap>) {
        let window = {
            let c = self.focus.get();
            c.get_focused_window()
        };

        (self.focus.clone(), window)
    }

    pub fn create_container(&mut self, layout: ContainerLayout) -> ContainerRef {
        let child = {
            let focused = self.get_focus();
            let current = focused.0.clone();
            let mut current = current.get_mut();
            current.create_child(layout, focused.0)
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

    pub fn map_all(
        &self,
        space: &mut Space,
        dh: &DisplayHandle,
        _renderer: &mut Gles2Renderer,
        _age: usize,
    ) {
        let root = self.root();
        let mut root = root.get_mut();
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
}
