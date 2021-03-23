use crate::config;
use bitvec::prelude::*;
use chrono::{DateTime, Utc};
use influxdb::Client;
use influxdb::InfluxDbWriteable;
use log::info;

use config::APP_CONFIG;
use std::collections::HashMap;
use std::fmt;

pub(crate) struct IngestionState {
    pub(crate) last_time: DateTime<Utc>,
    pub(crate) ingestion_vec: Vec<i64>,
    pub(crate) should_ingest: bool,
    pub(crate) recent_readings: HashMap<u8, Vec<WeatherReading>>,
}

#[derive(InfluxDbWriteable, Copy, Clone)]
pub struct WeatherReading {
    time: DateTime<Utc>,
    humidity: u8,
    temp_c: f64,
    temp_f: f64,
    #[influxdb(tag)]
    channel: u8,
}

impl PartialEq for WeatherReading {
    fn eq(&self, other: &Self) -> bool {
        self.humidity == other.humidity
            && (self.temp_c - other.temp_c).abs() < 0.1
            && self.channel == other.channel
    }
}
impl Eq for WeatherReading {}

impl fmt::Display for WeatherReading {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "({}, {}, {}, {})",
            self.time, self.temp_c, self.humidity, self.channel
        )
    }
}

pub(crate) fn handle_interrupt(influx_client: &Client, mut state: &mut IngestionState) {
    let new_time = Utc::now();
    let duration_micros = new_time
        .signed_duration_since(state.last_time)
        .num_microseconds()
        .unwrap();

    state.last_time = new_time;

    if duration_micros > (APP_CONFIG.last_sync_length_micros - APP_CONFIG.signal_variance_micros)
        && duration_micros
            < (APP_CONFIG.last_sync_length_micros + APP_CONFIG.signal_variance_micros)
    {
        state.should_ingest = false;

        let mut bit_vec: BitVec<Msb0, usize> = BitVec::new();

        state.ingestion_vec.iter().for_each(|&x| {
            if x > (APP_CONFIG.bit0_length_micros - APP_CONFIG.signal_variance_micros)
                && x < (APP_CONFIG.bit0_length_micros + APP_CONFIG.signal_variance_micros)
            {
                bit_vec.push(false);
            } else if x > (APP_CONFIG.bit1_length_micros - APP_CONFIG.signal_variance_micros)
                && x < (APP_CONFIG.bit1_length_micros + APP_CONFIG.signal_variance_micros)
            {
                bit_vec.push(true);
            } else {
                ()
            }
        });

        if bit_vec.len() == APP_CONFIG.expected_bit_length as usize {
            let temp: &BitSlice<Msb0, usize> = &bit_vec[16..28];
            let lhum: &BitSlice<Msb0, usize> = &bit_vec[28..32];
            let rhum: &BitSlice<Msb0, usize> = &bit_vec[32..36];
            let chan: &BitSlice<Msb0, usize> = &bit_vec[36..40];

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
                tempf_float = char_to_float(temp_string[0]) * 10.0
                    + char_to_float(temp_string[1])
                    + char_to_float(temp_string[2]) / 10.0;
            } else if temp_string.len() == 4 {
                let leftmost = char_to_float(temp_string[0]) + char_to_float(temp_string[1]);

                tempf_float = leftmost * 10.0
                    + char_to_float(temp_string[2])
                    + char_to_float(temp_string[3]) / 10.0;
            } else if temp_string.len() == 2 {
                tempf_float = char_to_float(temp_string[0]) + char_to_float(temp_string[1]) / 10.0;
            }

            let tempc_float = (tempf_float - 32.0) * 5.0 / 9.0;

            info!(
                "Processing bit vector of length 40: {}",
                bit_vec.to_string()
            );
            info!(
                "tempf: {}, tempc: {}, hum: {}, chan: {}",
                tempf_float, tempc_float, hum_num, chan
            );

            // When a new valid reading is received we want to do a few things:
            // Check the recent_readings map for each channel
            // if any has an entry that happened longer than five seconds ago take the following action
            // if there's only 1 entry, clear it
            // if there's multiple entries and they're all identical, take it otherwise clear it
            // after map is organised, input the chosen reading

            fn millis_since(target_time: DateTime<Utc>) -> i64 {
                Utc::now()
                    .signed_duration_since(target_time)
                    .num_milliseconds()
            }

            fn is_all_same(vec: &Vec<WeatherReading>) -> bool {
                vec.iter()
                    .fold((true, None), {
                        |acc: (bool, Option<&WeatherReading>), elem| {
                            if let Some(prev) = acc.1 {
                                (acc.0 && (*prev == *elem), Some(elem))
                            } else {
                                (true, Some(elem))
                            }
                        }
                    })
                    .0
            }

            for (ch, weather_vec) in state.recent_readings.iter_mut() {
                match weather_vec.iter().find(|&x| millis_since(x.time) >= 5000) {
                    Some(_) => {
                        // given that it's stale, start processing
                        if weather_vec.len() > 1 && is_all_same(weather_vec) {
                            let chosen_reading = weather_vec.first().unwrap();

                            info!(
                                "[ch: {}]: weather vec was greater than 1 and is all same, inserting: {}",
                                ch,
                                chosen_reading
                            );

                            async_std::task::block_on(async {
                                let _write_result = influx_client
                                    .query(&chosen_reading.into_query("weather"))
                                    .await;
                            });

                            weather_vec.clear();
                        } else {
                            info!(
                                "[ch: {}]: weather vec len: {} was too small or not the same, dumping",
                                ch,
                                weather_vec.len()
                            );
                            weather_vec.clear();
                        }
                    }
                    None => {
                        info!("[ch: {}]: not stale enough, doing nothing", ch)
                    }
                }
            }

            let weather_reading = WeatherReading {
                time: Utc::now(),
                humidity: hum_num,
                temp_c: tempc_float,
                temp_f: tempf_float,
                channel: chan,
            };

            let st_update: &mut Vec<WeatherReading> =
                state.recent_readings.entry(chan).or_insert(Vec::new());

            st_update.push(weather_reading.clone());
        }

        state.ingestion_vec.clear();
    }

    if state.should_ingest {
        state.ingestion_vec.push(duration_micros);
    }

    if state.ingestion_vec.len() >= 1024 {
        state.ingestion_vec.clear();
    }

    if duration_micros > (APP_CONFIG.first_sync_length_micros - APP_CONFIG.signal_variance_micros)
        && duration_micros
            < (APP_CONFIG.first_sync_length_micros + APP_CONFIG.signal_variance_micros)
    {
        // First sync indicates we should begin ingestion

        state.ingestion_vec.clear();
        state.should_ingest = true;
    }
}
