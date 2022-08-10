use crate::shell::container;
use crate::shell::container::{Container, ContainerLayout};
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::output::Output;
use std::cell::RefCell;
use std::rc::Rc;
use std::slice::Iter;
use smithay::reexports::ash::vk::Window;
use crate::shell::window::WindowWarp;

pub type ContainerRef = Rc<RefCell<Container>>;

#[derive(Debug)]
pub struct Tree {
    root: Rc<RefCell<Container>>,
    focus: Rc<RefCell<Container>>,
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
            windows: vec![],
            layout: ContainerLayout::Horizontal,
        };

        let root = Rc::new(RefCell::new(root));
        let focus = root.clone();

        Self { root, focus }
    }

    pub fn root(&self) -> ContainerRef {
        self.root.clone()
    }

    pub fn create_container(&mut self, layout: ContainerLayout) -> ContainerRef {
        let child = {
            let mut current = self.focus.borrow_mut();
            current.create_child(layout, self.focus.clone())
        };

        self.focus = child.clone();
        child
    }

    pub fn pop(&mut self) {
        let current = self.get_container_focused();
        let current = current.borrow();
        let id = current.id;
        if let Some(parent) = &current.parent {
            self.focus = parent.clone();
            let mut parent = parent.borrow_mut();
            let removed: Vec<ContainerRef> = parent
                .childs
                .drain_filter(|c| c.borrow().id == id)
                .collect();

            println!("Removed from parent {:?}", removed);
        }
    }

    pub fn get_container_focused(&self) -> ContainerRef {
        self.focus.clone()
    }

    pub fn flatten_window(&self) -> Vec<WindowWarp> {
        let root = self.root.borrow();
        let mut windows = root.windows.clone();

        for child in &root.childs {
            let window = child.borrow().flatten_window();
            windows.extend_from_slice(window.as_slice());
        }

        windows
    }
}
