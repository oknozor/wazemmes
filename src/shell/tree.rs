use crate::shell::container;
use crate::shell::container::{Container, ContainerLayout, ContainerRef};
use crate::shell::window::WindowWarp;
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::output::Output;

use std::collections::HashMap;


#[derive(Debug)]
pub struct Tree {
    root: ContainerRef,
    container_focused: ContainerRef,
}

impl Tree {
    pub fn new(output: &Output, geometry: Rectangle<i32, Logical>) -> Tree {
        let root = Container {
            id: container::id::get(),
            x: geometry.loc.x,
            y: geometry.loc.y,
            width: geometry.size.w,
            height: geometry.size.h,
            output: output.clone(),
            parent: None,
            childs: vec![],
            windows: HashMap::new(),
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
            let removed: Vec<ContainerRef> =
                parent.childs.drain_filter(|c| c.get().id == id).collect();

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
        let mut windows: Vec<WindowWarp> = root.windows.values().cloned().collect();

        for child in &root.childs {
            let window = child.get().flatten_window();
            windows.extend_from_slice(window.as_slice());
        }

        windows
    }

    pub fn flatten_containers(&self) -> Vec<ContainerRef> {
        let root = self.root.get();
        let mut root_children: Vec<ContainerRef> = root.childs.clone();

        for child in &root.childs {
            let children = child.get().flatten_containers();
            root_children.extend_from_slice(children.as_slice());
        }

        root_children
    }
}
