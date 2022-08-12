use crate::Tree;
use smithay::desktop::Space;
use smithay::reexports::wayland_server::DisplayHandle;
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
        Self {
            inner: Rc::new(RefCell::new(Workspace::new(output, space))),
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
    pub tree: Tree,
    pub output: Output,
}

impl Workspace {
    pub fn new(output: Output, space: &Space) -> Self {
        let geo = space.output_geometry(&output).unwrap();
        let tree = Tree::new(&output, geo);

        Self { tree, output }
    }

    pub fn unmap_all(&self, space: &mut Space) {
        for window in self.tree.flatten_window() {
            space.unmap_window(window.get());
        }
    }

    pub fn map_all(&self, space: &mut Space, dh: &DisplayHandle) {
        let root = self.tree.root();
        let mut root = root.get_mut();
        root.redraw(space);
        space.refresh(dh);
    }
}
