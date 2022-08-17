use crate::{Backend, Wazemmes};
use smithay::backend::renderer::utils::on_commit_buffer_handler;
use smithay::reexports::wayland_server::protocol::wl_buffer;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{CompositorHandler, CompositorState};
use smithay::wayland::shm::{ShmHandler, ShmState};
use smithay::{delegate_compositor, delegate_shm};

use super::xdg_shell;

impl<BackendData: Backend> CompositorHandler for Wazemmes<BackendData> {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn commit(&mut self, _dh: &DisplayHandle, surface: &WlSurface) {
        on_commit_buffer_handler(surface);
        self.space.commit(surface);

        xdg_shell::handle_commit(&self.space, surface);
    }
}

impl<BackendData: Backend> BufferHandler for Wazemmes<BackendData> {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl<BackendData: Backend> ShmHandler for Wazemmes<BackendData> {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(@<BackendData: 'static + Backend> Wazemmes<BackendData>);
delegate_shm!(@<BackendData: 'static + Backend> Wazemmes<BackendData>);
