use crate::backend::InputHandler;
use smithay::backend::input::InputEvent;
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::session::auto::AutoSession;
use smithay::backend::session::{Session, Signal as SessionSignal};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::input::Libinput;
use smithay::utils::signaling::{Linkable, Signaler};

/// Initialize libinput backend
pub fn init<D>(
    event_loop: LoopHandle<D>,
    mut session: AutoSession,
    session_signal: Signaler<SessionSignal>,
) where
    D: InputHandler,
{
    let mut libinput_context =
        Libinput::new_with_udev::<LibinputSessionInterface<AutoSession>>(session.clone().into());
    libinput_context.udev_assign_seat(&session.seat()).unwrap();

    let mut libinput_backend = LibinputInputBackend::new(libinput_context, None);
    libinput_backend.link(session_signal);

    event_loop
        .insert_source(libinput_backend, move |mut event, _, handler| {
            match &mut event {
                InputEvent::DeviceAdded { device } => {
                    device.config_tap_set_enabled(true).ok();
                }
                InputEvent::DeviceRemoved { .. } => {}
                _ => {}
            }

            handler.process_input_event(event, None, Some(&mut session));
        })
        .unwrap();
}
