use crate::shell::workspace::WorkspaceRef;
use crate::Wazemmes;

pub mod border;
pub mod container;
pub mod node;
pub mod nodemap;
pub mod window;
pub mod workspace;

impl Wazemmes {
    pub fn get_current_workspace(&self) -> WorkspaceRef {
        let current = &self.current_workspace;
        self.workspaces
            .get(current)
            .expect("Current workspace should exist")
            .clone()
    }

    pub fn move_to_workspace(&mut self, num: u8) {
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
            Some(workspace) => workspace.get_mut().update_layout(&self.space),
        };
    }
}
