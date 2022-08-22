use once_cell::sync::Lazy;

pub static CONFIG: Lazy<Config> = Lazy::new(Config::default);

pub struct Config {
    pub gaps: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self { gaps: 14 }
    }
}
