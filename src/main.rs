#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use] extern crate rocket;
extern crate config;

mod weather;

use std::error::Error;

use rppal::gpio::{Gpio, Trigger};
use rppal::system::DeviceInfo;
use chrono::prelude::*;
use bitvec::prelude::*;
use influxdb::Client;
use influxdb::InfluxDbWriteable;
use log::{debug, error, log_enabled, info, Level};

// Gpio uses BCM pin numbering.
const GPIO_RADIO: u8 = 17;

const BIT1_LENGTH: i64 = 3900;
const BIT0_LENGTH: i64 = 1800;
const FIRST_SYNC_LENGTH: i64 = 7900;
const LAST_SYNC_LENGTH: i64 = 15900;
const SIGNAL_VARIANCE: i64 = 500;

#[derive(InfluxDbWriteable)]
struct WeatherReading {
    time: DateTime<Utc>,
    humidity: u8,
    temp_c: f64,
    temp_f: f64,
    #[influxdb(tag)] channel: u8,
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let mut settings = config::Config::default();
    settings
        .merge(config::File::with_name("app_config")).unwrap()
        .merge(config::Environment::with_prefix("APP")).unwrap();

    info!("Started on {}.", DeviceInfo::new()?.model());

    let client = Client::new("http://localhost:8086", "rs_weather_sensors");
    let gpios = Gpio::new().unwrap();

    let mut ingestion_vec: Vec<i64> = Vec::new();

    let mut pin = gpios
        .get(GPIO_RADIO)
        .unwrap()
        .into_input_pulldown();

    let mut last_time = Utc::now();

    let mut should_ingest: bool = false;

    let pin_res = pin.set_async_interrupt(Trigger::Both, move |_| {

        let new_time = Utc::now();
        let duration_micros = new_time.signed_duration_since(last_time)
            .num_microseconds().unwrap();

        last_time = new_time;

        if duration_micros > (LAST_SYNC_LENGTH - SIGNAL_VARIANCE)
            && duration_micros < (LAST_SYNC_LENGTH + SIGNAL_VARIANCE) {

            should_ingest = false;

            let mut bit_vec: BitVec<Msb0, usize> = BitVec::new();

            ingestion_vec.iter().for_each(|&x| {

                if x > (BIT0_LENGTH - SIGNAL_VARIANCE)
                    && x < (BIT0_LENGTH + SIGNAL_VARIANCE) {
                    bit_vec.push(false);
                } else if x > (BIT1_LENGTH - SIGNAL_VARIANCE)
                    && x < (BIT1_LENGTH + SIGNAL_VARIANCE) {
                    bit_vec.push(true);
                } else {
                    ()
                }

            });

            if bit_vec.len() == 40 {
                // bits 16 to 28 are temp in weird encoding
                // bits 28 to 32 are lhum
                // bits 32 to 36 are rhum
                // bits 36 to 40 are the channel bits

                let temp: &BitSlice<Msb0, usize> = &bit_vec[16 .. 28];
                let lhum: &BitSlice<Msb0, usize> = &bit_vec[28 .. 32];
                let rhum: &BitSlice<Msb0, usize> = &bit_vec[32 .. 36];
                let chan: &BitSlice<Msb0, usize> = &bit_vec[36 .. 40];

                let tempf_num = temp.load::<u16>();
                let lhum_num = lhum.load::<u8>();
                let rhum_num = rhum.load::<u8>();
                let hum_num = lhum_num * 10 + rhum_num;
                let chan = chan.load::<u8>();

                let temp_string: Vec<char> = tempf_num.to_string().chars().collect();

                const RADIX: u32 = 10;

                fn char_to_float(char_to_convert: char) -> f64 {
                    char_to_convert.to_digit(RADIX).unwrap() as f64
                }

                let mut tempf_float: f64 = 0.0;
                if temp_string.len() == 3 {

                    tempf_float = char_to_float(temp_string[0]) * 10.0 +
                        char_to_float(temp_string[1]) +
                        char_to_float(temp_string[2]) / 10.0;

                } else if temp_string.len() == 4 {

                    let leftmost = char_to_float(temp_string[0]) +
                        char_to_float(temp_string[1]);

                    tempf_float = leftmost * 10.0 +
                        char_to_float(temp_string[2]) +
                        char_to_float(temp_string[3]) / 10.0;
                } else if temp_string.len() == 2 {
                    tempf_float = char_to_float(temp_string[0]) +
                        char_to_float(temp_string[1]) / 10.0;
                }

                let tempc_float = (tempf_float - 32.0) * 5.0/9.0;

                info!("Processing bit vector of length 40: {}", bit_vec.to_string());
                info!("tempf: {}, tempc: {}, hum: {}, chan: {}",
                         tempf_float, tempc_float, hum_num, chan);

                let weather_reading = WeatherReading {
                    time: Utc::now(),
                    humidity: hum_num,
                    temp_c: tempc_float,
                    temp_f: tempf_float,
                    channel: chan
                };

                async_std::task::block_on(async {
                    let _write_result = client
                        .query(&weather_reading.into_query("weather"))
                        .await;
                });

            }

            ingestion_vec.clear();
        }

        if should_ingest {
            ingestion_vec.push(duration_micros);
        }

        if ingestion_vec.len() >= 1024 {
            ingestion_vec.clear();
        }

        if duration_micros > (FIRST_SYNC_LENGTH - SIGNAL_VARIANCE)
            && duration_micros < (FIRST_SYNC_LENGTH + SIGNAL_VARIANCE) {

            // First sync indicates we should begin ingestion

            ingestion_vec.clear();
            should_ingest = true;

        }

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

    Ok(())
}