use crate::shell::container::{Container, ContainerLayout, ContainerRef};
use crate::shell::node;
use crate::shell::window::WindowWarp;
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::output::Output;

use crate::config::CONFIG;
use crate::shell::node::Node;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Tree {
    root: ContainerRef,
    container_focused: ContainerRef,
}

impl Tree {
    pub fn new(output: &Output, geometry: Rectangle<i32, Logical>) -> Tree {
        let gaps = CONFIG.gaps as i32;
        let root = Container {
            id: node::id::get(),
            x: geometry.loc.x + gaps,
            y: geometry.loc.y + gaps,
            width: geometry.size.w - (2 * gaps),
            height: geometry.size.h - (2 * gaps),
            output: output.clone(),
            parent: None,
            childs: HashMap::new(),
            layout: ContainerLayout::Horizontal,
            focus: None,
        };

        let root = ContainerRef::new(root);
        let focus = root.clone();

        Self {
            root,
            container_focused: focus,
        }
    }

    pub fn root(&self) -> ContainerRef {
        self.root.clone()
    }

    pub fn create_container(&mut self, layout: ContainerLayout) -> ContainerRef {
        let child = {
            let mut current = self.container_focused.get_mut();
            current.create_child(layout, self.container_focused.clone())
        };

        self.container_focused = child.clone();
        child
    }

    pub fn pop(&mut self) {
        let current = self.get_container_focused();
        let current = current.get();
        let id = current.id;
        if let Some(parent) = &current.parent {
            self.container_focused = parent.clone();
            let mut parent = parent.get_mut();
            let removed: Vec<(u32, Node)> = parent
                .childs
                .drain_filter(|node_id, _node| *node_id == id)
                .collect();

            println!("Removed from parent {:?}", removed);
        }
    }

    pub fn get_container_focused(&self) -> ContainerRef {
        self.container_focused.clone()
    }

    pub fn set_container_focused(&mut self, container: ContainerRef) {
        self.container_focused = container
    }

    pub fn flatten_window(&self) -> Vec<WindowWarp> {
        let root = self.root.get();
        let mut windows: Vec<WindowWarp> = root.iter_windows().cloned().collect();

        for child in root.iter_containers() {
            let window = child.get().flatten_window();
            windows.extend_from_slice(window.as_slice());
        }

        windows
    }
}
