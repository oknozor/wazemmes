use crate::backend::xwayland::X11State;
use crate::config::CONFIG;
use crate::shell::container::{Container, ContainerLayout, ContainerRef};
use crate::shell::drawable::{Border, Borders};
use crate::shell::node;
use crate::shell::node::Node;
use crate::shell::nodemap::NodeMap;
use crate::shell::windows::toplevel::WindowWrap;
use slog_scope::debug;
use smithay::desktop::Space;
use smithay::utils::{Logical, Physical, Rectangle};
use smithay::wayland::output::Output;
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct WorkspaceRef {
    inner: Rc<RefCell<Workspace>>,
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
    pub(crate) needs_redraw: bool,
    pub borders: Vec<Borders>,
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
            #[cfg(feature = "xwayland")]
            xpopups: vec![],
        };

        let root = ContainerRef::new(root);
        let focus = root.clone();

        Self {
            output: output.clone(),
            root,
            focus,
            fullscreen_layer: None,
            needs_redraw: false,
            borders: vec![],
        }
    }

    pub fn update_layout(&mut self, space: &Space) {
        let geometry = space.output_geometry(&self.output).unwrap();
        let root = &self.root;
        let mut root = root.get_mut();
        self.needs_redraw = root.update_layout(geometry);
    }

    pub fn redraw(&mut self, space: &mut Space, x11_state: Option<&mut X11State>) {
        let geometry = space.output_geometry(&self.output).expect("Geometry");
        self.unmap_all(space);

        if let Some(layer) = &self.fullscreen_layer {
            match layer {
                Node::Container(container) => {
                    debug!("Redraw: FullScreen Container");
                    let container = container.get();
                    container.redraw(space, x11_state);
                }
                Node::Window(window) => {
                    debug!("Redraw: FullScreen Window");
                    window.set_fullscreen(geometry);
                    window.map(space, x11_state, true);
                }
            }
        } else {
            debug!("Redraw: Root Container");
            let root = self.root.get();
            root.redraw(space, x11_state);
        }

        self.needs_redraw = false;
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

    pub fn set_container_focused(&mut self, container: &ContainerRef) {
        self.focus = container.clone();
    }

    pub fn set_container_and_window_focus(
        &mut self,
        container: &ContainerRef,
        window: &WindowWrap,
    ) {
        self.focus = container.clone();
        container.get_mut().set_focus(window.id());
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

    pub fn unmap_all(&mut self, space: &mut Space) {
        self.borders.drain(..);
        for window in self.flatten_window() {
            space.unmap_window(window.inner());
        }
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
        let mut container = self.root.get_mut();
        container.location = (geometry.loc.x + gaps, geometry.loc.y + gaps).into();
        container.size = (geometry.size.w - 2 * gaps, geometry.size.h - 2 * gaps).into();
    }

    pub fn get_output_geometry_f64(&self, space: &Space) -> Option<Rectangle<f64, Physical>> {
        space.output_geometry(&self.output).map(|geometry| {
            let scale = self.output.current_scale().fractional_scale();
            geometry.to_f64().to_physical_precise_up(scale)
        })
    }

    pub fn update_borders(&mut self) {
        debug!("Updating workspace borders");
        match &self.fullscreen_layer {
            Some(Node::Container(container)) => {
                let container = container.get();
                let container_borders = container.make_borders();
                let window_borders = container
                    .get_focused_window()
                    .map(|window| window.get_state().borders());

                self.borders = vec![container_borders];

                if let Some(window_borders) = window_borders {
                    self.borders.push(window_borders);
                }
            }
            Some(Node::Window(_)) => {
                // No border for window fullscreen mode
            }
            None => {
                let (container, window) = self.get_focus();
                let container = container.get();
                let container_borders = container.make_borders();
                let window_borders = window.map(|window| window.get_state().borders());

                self.borders = vec![container_borders];

                if let Some(window_borders) = window_borders {
                    self.borders.push(window_borders);
                }
            }
        }
    }
}
