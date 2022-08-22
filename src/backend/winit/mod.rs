use crate::backend::{BackendHandler, NewOutputDescriptor, OutputId};
use slog_scope::{error, info};
use smithay::backend::winit;
use smithay::backend::winit::WinitEvent;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::protocol::wl_output;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::output::{Mode, PhysicalProperties};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub const OUTPUT_NAME: &str = "winit";

pub fn run_winit<D>(
    event_loop: &mut EventLoop<'static, D>,
    _display: &DisplayHandle,
    handler: &mut D,
) -> Result<(), ()>
where
    D: BackendHandler + 'static,
{
    let (backend, mut input) = winit::init(None).map_err(|err| {
        error!("Failed to initialize Winit backend: {}", err);
    })?;

    let backend = Rc::new(RefCell::new(backend));

    let size = backend.borrow().window_size().physical_size;

    /*
     * Initialize the globals
     */

    let mode = Mode {
        size,
        refresh: 60_000,
    };

    let physical_properties = PhysicalProperties {
        size: (0, 0).into(),
        subpixel: wl_output::Subpixel::Unknown,
        make: "Smithay".into(),
        model: "Winit".into(),
    };

    let output = NewOutputDescriptor {
        id: OutputId { id: 1 },
        physical_properties,
        transform: wl_output::Transform::Flipped180,
        name: OUTPUT_NAME.to_owned(),
        prefered_mode: mode,
        possible_modes: vec![mode],
    };

    let output_id = output.id;
    handler.output_created(output);
    handler.start_compositor();

    info!("Initialization completed, starting the main loop.");

    let timer = Timer::immediate();

    event_loop
        .handle()
        .insert_source(timer, move |_, _, handler| {
            let res = input.dispatch_new_events(|event| match event {
                WinitEvent::Resized { size, .. } => {
                    let mode = Mode {
                        size,
                        refresh: 60_000,
                    };

                    handler.output_mode_updated(&output_id, mode);
                }
                WinitEvent::Input(event) => {
                    handler.process_input_event(event, Some(&output_id), None);
                }
                _ => {}
            });

            match res {
                Ok(()) => {
                    let mut backend = backend.borrow_mut();

                    if backend.bind().is_ok() {
                        let age = backend.buffer_age().unwrap_or(0);
                        let damage = handler
                            .output_render(backend.renderer(), &output_id, age, None)
                            .unwrap();
                        backend.submit(damage.as_deref()).unwrap();
                    }

                    handler.send_frames(&output_id);

                    TimeoutAction::ToDuration(Duration::from_millis(16))
                }
                Err(winit::WinitError::WindowClosed) => {
                    handler.close_compositor();

                    TimeoutAction::Drop
                }
            }
        })
        .unwrap();

    Ok(())
}
