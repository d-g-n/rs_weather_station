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
use influxdb::InfluxDbWriteable;
use influxdb::{Client, Timestamp, WriteQuery};
use log::{error, info};
use rocket::http::Status;
use rocket::{Data, State};
use rppal::gpio::{Gpio, Trigger};
use rppal::system::DeviceInfo;
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[post("/esp", format = "json", data = "<data>")]
fn esp_post(influx_client: State<Client>, data: Data) -> Status {
    let mut buffer: String = String::new();

    data.open().read_to_string(&mut buffer).unwrap();

    let v: Value = serde_json::from_str(&*buffer).unwrap();

    let rt: WriteQuery =
        Timestamp::Milliseconds(Utc::now().timestamp_millis() as u128).into_query("esp");

    let query_to_write: WriteQuery = v.as_object().unwrap().iter().fold(rt, |wq, (key, value)| {
        if value.is_f64() {
            wq.add_field(key, value.as_f64().unwrap() as f32)
        } else if value.is_i64() {
            wq.add_field(key, value.as_i64().unwrap() as f32)
        } else if value.is_u64() {
            wq.add_field(key, value.as_u64().unwrap() as f32)
        } else {
            wq
        }
    });

    async_std::task::block_on(async {
        let write_result = influx_client.query(&query_to_write).await;
        info!("Influx Query Res: {:?}", write_result);
    });

    info!("Json Body: {:?}", v);

    Status::Ok
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

    let client_other = Client::new(
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

    rocket::ignite()
        .manage(client_other)
        .mount("/api", routes![index, esp_post])
        .launch();

    info!(
        "Started rs_weather_station on {}.",
        DeviceInfo::new()?.model()
    );

    Ok(())
}
