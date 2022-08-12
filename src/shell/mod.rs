use crate::shell::workspace::WorkspaceRef;
use crate::Wazemmes;
use smithay::desktop::Window;
use smithay::reexports::wayland_server::DisplayHandle;

use std::cell::RefCell;

pub mod container;
pub mod node;
pub mod window;
pub mod workspace;
pub mod nodemap;

#[derive(Default)]
pub struct FullscreenSurface(RefCell<Option<Window>>);

impl FullscreenSurface {
    pub fn set(&self, window: Window) {
        *self.0.borrow_mut() = Some(window);
    }
    pub fn get(&self) -> Option<Window> {
        self.0.borrow().clone()
    }
    pub fn clear(&self) -> Option<Window> {
        self.0.borrow_mut().take()
    }
}

impl<Backend> Wazemmes<Backend> {
    pub fn get_current_workspace(&self) -> WorkspaceRef {
        let current = &self.current_workspace;
        self.workspaces
            .get(current)
            .expect("Current workspace should exist")
            .clone()
    }

    pub fn move_to_workspace(&mut self, num: u8, dh: &DisplayHandle) {
        // Target workspace is already focused
        if self.current_workspace == num {
            return;
        }

        let current_workspace = self.get_current_workspace();
        let current_workspace = current_workspace.get_mut();
        current_workspace.unmap_all(&mut self.space);
        self.current_workspace = num;

        match self.workspaces.get(&num) {
            None => {
                let output = self.space.outputs().next().unwrap();
                let workspace = WorkspaceRef::new(output.clone(), &self.space);
                self.workspaces.insert(num, workspace);
            }
            Some(workspace) => workspace.get_mut().map_all(&mut self.space, dh),
        };

        self.space.refresh(dh);
    }
}
