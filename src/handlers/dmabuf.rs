use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::delegate_dmabuf;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportError};

use crate::{CallLoopData, Wazemmes};

impl DmabufHandler for Wazemmes {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        global: &DmabufGlobal,
        dmabuf: Dmabuf,
    ) -> Result<(), ImportError> {
        self.backend.dmabuf_imported(&self.display, global, dmabuf)
    }
}

impl AsMut<DmabufState> for CallLoopData {
    fn as_mut(&mut self) -> &mut DmabufState {
        self.state.dmabuf_state()
    }
}

delegate_dmabuf!(Wazemmes);
