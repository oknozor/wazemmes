use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_shm,
    reexports::wayland_server::{
        protocol::{wl_buffer, wl_surface::WlSurface},
        DisplayHandle,
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorHandler, CompositorState},
        shm::{ShmHandler, ShmState},
    },
};
use crate::Wazemmes;

use super::xdg_shell;

impl CompositorHandler for Wazemmes {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn commit(&mut self, _dh: &DisplayHandle, surface: &WlSurface) {
        on_commit_buffer_handler(surface);
        self.space.commit(surface);

        xdg_shell::handle_commit(&self.space, surface);
    }
}

impl BufferHandler for Wazemmes {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for Wazemmes {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(Wazemmes);
delegate_shm!(Wazemmes);
