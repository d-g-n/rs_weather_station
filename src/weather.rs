use crate::config;
use bitvec::prelude::*;
use chrono::{DateTime, Utc};
use influxdb::Client;
use influxdb::InfluxDbWriteable;
use log::info;

use config::APP_CONFIG;

pub(crate) struct IngestionState {
    pub(crate) last_time: DateTime<Utc>,
    pub(crate) ingestion_vec: Vec<i64>,
    pub(crate) should_ingest: bool,
}

#[derive(InfluxDbWriteable)]
struct WeatherReading {
    time: DateTime<Utc>,
    humidity: u8,
    temp_c: f64,
    temp_f: f64,
    #[influxdb(tag)]
    channel: u8,
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

            let weather_reading = WeatherReading {
                time: Utc::now(),
                humidity: hum_num,
                temp_c: tempc_float,
                temp_f: tempf_float,
                channel: chan,
            };

            async_std::task::block_on(async {
                let _write_result = influx_client
                    .query(&weather_reading.into_query("weather"))
                    .await;
            });
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
