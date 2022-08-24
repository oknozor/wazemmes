use crate::backend::BackendHandler;
use crate::{BackendState, CallLoopData, Wazemmes, WorkspaceRef};
use smithay::wayland::dmabuf::DmabufState;

impl BackendHandler for CallLoopData {
    type WaylandState = Wazemmes;

    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.state.dmabuf_state
    }

    fn backend_state(&mut self) -> &mut BackendState {
        &mut self.state.backend
    }

    #[cfg(feature = "xwayland")]
    fn start_xwayland(&mut self) {
        self.state.start_xwayland()
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
