use crate::Wazemmes;
use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::reexports::wayland_server::protocol::wl_buffer;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{CompositorHandler, CompositorState};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{delegate_compositor, delegate_shm};

use super::xdg_shell;

impl<Backend> CompositorHandler for Wazemmes<Backend> {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn commit(&mut self, _dh: &DisplayHandle, surface: &WlSurface) {
        on_commit_buffer_handler(surface);
        self.space.commit(surface);

        xdg_shell::handle_commit(&self.space, surface);
    }
}

impl<Backend> BufferHandler for Wazemmes<Backend> {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl<Backend> ShmHandler for Wazemmes<Backend> {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(@<BackendData: 'static> Wazemmes<BackendData>);
delegate_shm!(@<BackendData: 'static> Wazemmes<BackendData>);
