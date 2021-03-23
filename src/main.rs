#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;

extern crate lazy_static;

mod config;
use config::APP_CONFIG;
mod weather;

use weather::IngestionState;

use std::error::Error;

use chrono::prelude::*;
use influxdb::Client;
use log::{error, info};
use rppal::gpio::{Gpio, Trigger};
use rppal::system::DeviceInfo;
use std::collections::HashMap;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let client = Client::new(
        format!(
            "http://{}:{}",
            APP_CONFIG.influx_host, APP_CONFIG.influx_port
        ),
        APP_CONFIG.influx_database.clone(),
    );

    let mut pin = Gpio::new()
        .unwrap()
        .get(APP_CONFIG.gpio_radio_pin_bcm)
        .unwrap()
        .into_input_pulldown();

    let mut ingestion_state = IngestionState {
        last_time: Utc::now(),
        ingestion_vec: Vec::new(),
        should_ingest: false,
        recent_readings: HashMap::new(),
    };

    let pin_res = pin.set_async_interrupt(Trigger::Both, move |_| {
        weather::handle_interrupt(&client, &mut ingestion_state);

        ()
    });

    match pin_res {
        Ok(()) => {
            info!("Registered GPIO pin okay");
        }
        Err(err) => {
            error!("Could not register pin: {:?}", err);
        }
    }

    rocket::ignite().mount("/", routes![index]).launch();

    info!(
        "Started rs_weather_station on {}.",
        DeviceInfo::new()?.model()
    );

    Ok(())
}
