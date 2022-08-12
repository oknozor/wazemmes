#![feature(drain_filter)]
#![feature(hash_drain_filter)]

extern crate core;

use crate::shell::tree::Tree;
use crate::state::{Backend, Wazemmes};
use crate::winit::WinitData;
use slog::{o, Drain};
use smithay::reexports::wayland_server::Display;

mod handlers;
mod inputs;
mod shell;
mod state;
// mod udev;
mod config;
mod drawing;
mod render;
mod winit;

pub struct CallLoopData<BackendData: 'static> {
    state: Wazemmes<BackendData>,
    display: Display<Wazemmes<BackendData>>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let log = slog::Logger::root(
        slog_async::Async::default(slog_term::term_full().fuse()).fuse(),
        o!(),
    );

    let _guard = slog_scope::set_global_logger(log.clone());
    slog_stdlog::init().expect("Could not setup log backend");

    winit::init_winit(log);

    Ok(())
}
