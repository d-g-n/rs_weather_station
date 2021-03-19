use std::error::Error;
use std::thread;
use std::time::Duration;

use rppal::gpio::{Gpio, Trigger, Level};
use rppal::system::DeviceInfo;

// Gpio uses BCM pin numbering.
const GPIO_RADIO: u8 = 17;

fn handle(level: Level) -> Result<(), Box<dyn Error>> {

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("Blinking an LED on a {}.", DeviceInfo::new()?.model());

    let mut pin = Gpio::new()?.get(GPIO_RADIO)?.into_input()
        .set_async_interrupt(Trigger::Both, handle);

    // Blink the LED by setting the pin's logic level high for 500 ms.
    pin.set_high();
    thread::sleep(Duration::from_millis(500));
    pin.set_low();

    Ok(())
}