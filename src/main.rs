#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;

use std::error::Error;

use rppal::gpio::{Gpio, Trigger, Level};
use rppal::system::DeviceInfo;
use ringbuf::RingBuffer;
use chrono::prelude::*;

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

    let gpios = Gpio::new().unwrap();
    let rb: RingBuffer<i64> = RingBuffer::<i64>::new(RING_BUFFER_SIZE);

    let mut ingestion_vec: Vec<i64> = Vec::new();


    let (mut prod, mut cons) = rb.split();

    let mut pin = gpios
        .get(GPIO_RADIO)
        .unwrap()
        .into_input_pulldown();

    let mut last_time = Utc::now();

    let mut should_ingest: bool = false;

    let pin_res = pin.set_async_interrupt(Trigger::Both, move |level: Level| {
        //println!("received level {:?} ", level);

        let new_time = Utc::now();
        let duration_micros = new_time.signed_duration_since(last_time)
            .num_microseconds().unwrap();
        last_time = new_time;
        let push_res = prod.push(duration_micros);

        //println!("received calculated duration {:?}", duration_micros);

        if duration_micros > (FIRST_SYNC_LENGTH - 1000)
            && duration_micros < (FIRST_SYNC_LENGTH + 1000) {
            println!("received calculated duration {:?}", duration_micros);
            println!("observed sync signal");
            // First sync indicates we should begin ingestion

            ingestion_vec.clear();
            should_ingest = true;

        }

        if duration_micros > (LAST_SYNC_LENGTH - 1000)
            && duration_micros < (LAST_SYNC_LENGTH + 1000) {
            println!("received calculated duration {:?}", duration_micros);
            println!("observed last sync signal");

            println!("ingestion length was: {}", ingestion_vec.len());

            should_ingest = false;

            let bit_vec = ingestion_vec.iter().flat_map(|&x| {

                if x > (BIT0_LENGTH - 1000)
                    && x < (BIT0_LENGTH + 1000) {
                    Some("0")
                } else if x > (BIT1_LENGTH - 1000)
                    && x < (BIT1_LENGTH + 1000) {
                    Some("1")
                } else {
                    None
                }

            }).collect::<Vec<_>>();

            println!("generated bits are: {}", bit_vec.join(""));

            ingestion_vec.clear();
        }

        if should_ingest {
            ingestion_vec.push(duration_micros);
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