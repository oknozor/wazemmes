use eyre::{ eyre};
use slog_scope::debug;
use smithay::backend::allocator::Buffer;
use smithay::backend::drm::DrmNode;
use smithay::backend::egl::{EGLDevice};
use smithay::backend::renderer::{ExportDma};
use smithay::backend::renderer::gles2::{Gles2Renderer};
use smithay::output::Output;

use smithay::reexports::wayland_server::{Client, DataInit, Dispatch, GlobalDispatch, New};
use smithay::reexports::wayland_server::backend::GlobalId;
use wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
use wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_manager_v1::Request as CaptureRequest;
use wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1;
use wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_frame_v1::Request as FrameRequest;

use crate::{BackendState, DisplayHandle, Wazemmes};
use crate::backend::drawing::AsGles2Renderer;

pub struct ScreenCopyManagerState {
    global: GlobalId,
}

impl ScreenCopyManagerState {
    pub(crate) fn new(dh: &DisplayHandle) -> Self {
        ScreenCopyManagerState {
            global: dh.create_global::<Wazemmes, ZwlrScreencopyManagerV1, ScreenCopyManagerGlobalData>(3, ScreenCopyManagerGlobalData {
                filter: Box::new(|_| true)
            })
        }
    }
}

pub struct ScreenCopyManagerGlobalData {
    filter: Box<dyn for<'a> Fn(&'a Client) -> bool + Send + Sync>,
}

impl GlobalDispatch<ZwlrScreencopyManagerV1, ScreenCopyManagerGlobalData> for Wazemmes {
    fn bind(
        state: &mut Wazemmes,
        handle: &DisplayHandle,
        client: &Client, resource: New<ZwlrScreencopyManagerV1>,
        global_data: &ScreenCopyManagerGlobalData,
        data_init: &mut DataInit<'_, Wazemmes>) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for Wazemmes {
    fn request(state: &mut Self, client: &Client, resource: &ZwlrScreencopyManagerV1, request: CaptureRequest, data: &(), dh: &DisplayHandle, data_init: &mut DataInit<'_, Self>) {
        // See: https://wayland.app/protocols/wlr-screencopy-unstable-v1
        match request {
            CaptureRequest::CaptureOutput { frame, overlay_cursor, output } => {
                debug!("Capture output request: Requestframe={frame:?}, overlay_cursor={overlay_cursor:?}, output={output:?}");
                let output = Output::from_resource(&output)
                    .ok_or(eyre!("Output is gone"))
                    .unwrap();

                let frame = data_init.init(frame, ());

                let drm = match &state.backend {
                    BackendState::Drm(drm) => drm,
                    BackendState::None => panic!("Screenshot not supported for winit backend"),
                };

                let mut renderer = drm.gpu_manager.borrow_mut();
                let primary_gpu = &drm.primary_gpu;
                let mut renderer = renderer.renderer(primary_gpu, primary_gpu);
                let renderer = renderer.as_mut().unwrap()
                    .as_gles2();

                let size = state.space.output_geometry(&output).unwrap().size.to_f64().to_buffer(
                    output.current_scale().fractional_scale(),
                    output.current_transform().into(),
                ).to_i32_round();

                let dmabuf = renderer.export_framebuffer(size);
                let dmabuf = dmabuf.expect("Failed to export buffer");


                let w = (size.w as i32).try_into().unwrap();
                let h = (size.h as i32).try_into().unwrap();
                let format = dmabuf.format().code;
                debug!("Trying to send screen buffer info h={h}, w={w}, format={format}");

                frame.linux_dmabuf(format as u32, w, h);
                frame.buffer_done();
            }
            CaptureRequest::CaptureOutputRegion {
                frame,
                overlay_cursor,
                output,
                x,
                y,
                width,
                height
            } => {
                todo!()
            }
            CaptureRequest::Destroy => {
                todo!()
            }
            _ => unimplemented!()
        };
    }
}

pub struct ScreenCopyFrame {
    frame: ZwlrScreencopyFrameV1,
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for Wazemmes {
    fn request(state: &mut Self, client: &Client, resource: &ZwlrScreencopyFrameV1, request: FrameRequest, data: &(), dhandle: &DisplayHandle, data_init: &mut DataInit<'_, Self>) {
        debug!("Frame request {request:?}");
    }
}

fn device_from_renderer(renderer: &Gles2Renderer) -> eyre::Result<DrmNode> {
    EGLDevice::device_for_display(renderer.egl_context().display())?
        .try_get_render_node()?
        .ok_or(eyre!("No node associated with context (software context?)"))
}

