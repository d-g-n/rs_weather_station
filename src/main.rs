#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;

use std::error::Error;

use rppal::gpio::{Gpio, Trigger, Level};
use rppal::system::DeviceInfo;
use chrono::prelude::*;
use bitvec::prelude::*;
use influxdb::{Client, Timestamp};
use influxdb::InfluxDbWriteable;

// Gpio uses BCM pin numbering.
const GPIO_RADIO: u8 = 17;

/*
high 	- low 	- high 	- low 	- high 	- low 	- high 	- low 	- high 	- sync 	- 40 bits 	- end sample?
48s  	- 41s 	- 46s  	- 42s 	- 47s  	- 42s 	- 47s  	- 42s 	- 27s  	- 345s 	- ???     	- 702s
1.09ms 	- 0.92	- 1.04	- 0.95	- 1.07 	- 0.95	- 1.07	- 0.95	- 0.61	- 7.82 	- ???		- 15.92
 */

const BIT1_LENGTH: i64 = 3900;
const BIT0_LENGTH: i64 = 1800;
const FIRST_SYNC_LENGTH: i64 = 7900;
const LAST_SYNC_LENGTH: i64 = 15900;

const RING_BUFFER_SIZE: usize = 256;


#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}


fn main() -> Result<(), Box<dyn Error>> {
    println!("Started {}.", DeviceInfo::new()?.model());

    let client = Client::new("http://localhost:8086", "rs_weather_sensors");
    let gpios = Gpio::new().unwrap();


    #[derive(InfluxDbWriteable)]
    struct WeatherReading {
        time: DateTime<Utc>,
        humidity: u8,
        temp_c: f64,
        temp_f: f64,
        channel: u8,
    }

    let mut ingestion_vec: Vec<i64> = Vec::new();

    let mut pin = gpios
        .get(GPIO_RADIO)
        .unwrap()
        .into_input_pulldown();

    let mut last_time = Utc::now();

    let mut should_ingest: bool = false;

    let pin_res = pin.set_async_interrupt(Trigger::Both, move |_level: Level| {
        //println!("received level {:?} ", level);

        let new_time = Utc::now();
        let duration_micros = new_time.signed_duration_since(last_time)
            .num_microseconds().unwrap();
        last_time = new_time;


        //println!("received calculated duration {:?}", duration_micros);

        if duration_micros > (LAST_SYNC_LENGTH - 1000)
            && duration_micros < (LAST_SYNC_LENGTH + 1000) {
            println!("received calculated duration {:?}", duration_micros);
            println!("observed last sync signal");

            println!("ingestion length was: {}", ingestion_vec.len());

            should_ingest = false;

            let mut bit_vec: BitVec<Msb0, usize> = BitVec::new();

            ingestion_vec.iter().for_each(|&x| {

                if x > (BIT0_LENGTH - 1000)
                    && x < (BIT0_LENGTH + 1000) {
                    bit_vec.push(false);
                } else if x > (BIT1_LENGTH - 1000)
                    && x < (BIT1_LENGTH + 1000) {
                    bit_vec.push(true);
                } else {
                    ()
                }

            });

            println!("bit vector is: {}, length is: {}", bit_vec.to_string(), bit_vec.len());

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

                fn char_to_float(blah: char) -> f64 {
                    blah.to_digit(RADIX) as f64
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
                }

                println!("tempf_float: {}", tempf_float);

                println!("tempf_bin: {}", temp.to_string());

                println!("tempf: {}, lhum: {}, rhum: {}, hum: {}, chan: {}",
                         tempf_num, lhum_num, rhum_num, hum_num, chan);

                let weather_reading = WeatherReading {
                    time: Utc::now(),
                    humidity: hum_num,
                    temp_c: 10.0,
                    temp_f: 10.0,
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

        if duration_micros > (FIRST_SYNC_LENGTH - 1000)
            && duration_micros < (FIRST_SYNC_LENGTH + 1000) {

            // First sync indicates we should begin ingestion

            ingestion_vec.clear();
            should_ingest = true;

        }

        ()
    });

    match pin_res {
        Ok(()) => {
            println!("Registered GPIO pin okay");
        }
        Err(err) => {
            println!("Could not register pin: {:?}", err);
        }
    }

    rocket::ignite().mount("/", routes![index]).launch();

    Ok(())
}