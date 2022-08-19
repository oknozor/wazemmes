use crate::backend::PreferedBackend;
use clap::Parser;

/// Rust wayland compositor
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct WazemmesCli {
    /// Selected backend: auto, x11, winit, udev
    #[clap(short, long, default_value = "auto")]
    pub backend: PreferedBackend,
}
