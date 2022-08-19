use crate::backend::BackendHandler;
use crate::{BackendState, CallLoopData, Wazemmes, WorkspaceRef};

impl BackendHandler for CallLoopData {
    type WaylandState = Wazemmes;

    fn backend_state(&mut self) -> &mut BackendState {
        &mut self.state.backend
    }

    fn send_frames(&mut self) {
        self.state
            .space
            .send_frames(self.state.start_time.elapsed().as_millis() as u32);
    }

    fn start_compositor(&mut self) {
        ::std::env::set_var("WAYLAND_DISPLAY", &self.state.socket_name);

        if let Some(output) = self.state.space.outputs().next() {
            self.state
                .workspaces
                .insert(0, WorkspaceRef::new(output.clone(), &self.state.space));
        } else {
            panic!("Failed to create Workspace 0 on default Output");
        }

        dbg!(&self.state.socket_name);
    }

    fn close_compositor(&mut self) {
        self.state.loop_signal.stop();
    }
}
