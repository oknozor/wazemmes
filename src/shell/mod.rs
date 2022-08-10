use std::cell::RefMut;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::shell::xdg::XdgShellHandler;
use crate::shell::workspace::{Workspace, WorkspaceRef};
use crate::Wazemmes;

pub mod container;
pub mod tree;
pub mod window;
pub mod workspace;

impl Wazemmes {
    pub fn get_current_workspace(&self) -> WorkspaceRef {
        let current = &self.current_workspace;
        self.workspaces.get(current)
            .expect("Current workspace should exist")
            .clone()
    }

    pub fn new_workspace(&mut self, workspace_id: u8) {
        let output = self.space.outputs().next().unwrap();
        let workspace = Workspace::new(output.clone(), &self.space);
        self.workspaces.insert(workspace_id, WorkspaceRef::from(workspace));
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
            Some(workspace) => {
                workspace.get_mut().map_all(&mut self.space, dh)
            }
        };

        self.space.refresh(dh);
    }
}