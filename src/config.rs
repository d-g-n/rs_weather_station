use lazy_static::lazy_static;
use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Debug)]
pub struct AppConfig {
    pub gpio_radio_pin_bcm: u8,
    pub bit1_length_micros: i64,
    pub bit0_length_micros: i64,
    pub first_sync_length_micros: i64,
    pub last_sync_length_micros: i64,
    pub signal_variance_micros: i64,
    pub expected_bit_length: i64,
    pub influx_host: String,
    pub influx_port: i64,
    pub influx_database: String,
}

lazy_static! {
    pub static ref APP_CONFIG: AppConfig = get_config();
}

fn get_config() -> AppConfig {
    let configs: AppConfig = toml::from_slice(&fs::read("./app_config.toml").unwrap()).unwrap();

    configs
}
