use crate::shell::container;
use crate::shell::container::{Container, ContainerLayout};
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::output::Output;
use std::cell::RefCell;
use std::rc::Rc;

pub type ContainerRef = Rc<RefCell<Container>>;

pub struct Focus {
    container: Rc<RefCell<Container>>,
}

impl Focus {
    fn init(root: Rc<RefCell<Container>>) -> Focus {
        Self { container: root }
    }
}

pub struct Tree {
    focus: Focus,
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
            surfaces: vec![],
            layout: ContainerLayout::Horizontal,
        };

        let root = Rc::new(RefCell::new(root));

        Self { focus: Focus::init(root) }
    }

    pub fn create_container(&mut self, layout: ContainerLayout) -> ContainerRef {
        let child = {
            let mut current = self.focus.container.borrow_mut();
            current.create_child(layout, self.focus.container.clone())
        };

        self.focus.container = child.clone();
        child
    }

    pub fn pop(&mut self) {
        let current = self.get_container_focused();
        let current = current.borrow();
        let id = current.id;
        if let Some(parent) = &current.parent {
            self.focus.container = parent.clone();
            let mut parent = parent.borrow_mut();
            let removed: Vec<ContainerRef> = parent
                .childs
                .drain_filter(|c| c.borrow().id == id)
                .collect();

            println!("Removed from parent {:?}", removed);
        }
    }

    pub fn get_container_focused(&self) -> ContainerRef {
        self.focus.container.clone()
    }
}
