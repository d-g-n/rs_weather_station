#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;

use std::error::Error;

use rppal::gpio::{Gpio, Trigger, Level};
use rppal::system::DeviceInfo;
use ringbuf::RingBuffer;

// Gpio uses BCM pin numbering.
const GPIO_RADIO: u8 = 17;

/*
high 	- low 	- high 	- low 	- high 	- low 	- high 	- low 	- high 	- sync 	- 40 bits 	- end sample?
48s  	- 41s 	- 46s  	- 42s 	- 47s  	- 42s 	- 47s  	- 42s 	- 27s  	- 345s 	- ???     	- 702s
1.09ms 	- 0.92	- 1.04	- 0.95	- 1.07 	- 0.95	- 1.07	- 0.95	- 0.61	- 7.82 	- ???		- 15.92
 */

const BIT1_LENGTH: u32 = 3900;
const BIT0_LENGTH: u32 = 1800;
const FIRST_SYNC_LENGTH: u32 = 7900;
const LAST_SYNC_LENGTH: u32 = 15900;

const RING_BUFFER_SIZE: usize = 256;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}


fn main() -> Result<(), Box<dyn Error>> {
    println!("Started {}.", DeviceInfo::new()?.model());
    let rb: RingBuffer<i32> = RingBuffer::<i32>::new(RING_BUFFER_SIZE);


    let mut pin = Gpio::new()?.get(GPIO_RADIO)?.into_input()
        .set_async_interrupt(Trigger::Both, |level: Level| {
            println!("received level {} ", level);

        });

    match pin {
        Ok(()) => {
            println!("Registered GPIO pin okay");
        }
        Err(err) => {
            println!("Could not register pin: {:?}", err)
        }
    }

    rocket::ignite().mount("/", routes![index]).launch();

    Ok(())
}