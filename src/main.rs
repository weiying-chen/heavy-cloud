use esp_idf_hal::gpio::{Gpio2, Gpio3};
use esp_idf_hal::prelude::Peripherals;
use esp_idf_svc::log::EspLogger;
use esp_idf_sys::{self as _};
use log::{info, warn};
use std::time::Duration; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
mod critical_section;
mod net;
mod scale;
use crate::net::{Http, Wifi};
use crate::scale::Scale;
const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASSWORD: &str = env!("WIFI_PASS");
const SUPABASE_KEY: &str = env!("SUPABASE_KEY");
const SUPABASE_URL: &str = env!("SUPABASE_URL");
const LOAD_SENSOR_SCALING: f32 = 0.0027;

#[link_section = ".rtc.data"]
static mut TARE_OFFSET: i32 = 0;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let dt = peripherals.pins.gpio2;
    let sck = peripherals.pins.gpio3;
    let mut scale = Scale::new(sck, dt, LOAD_SENSOR_SCALING).unwrap();

    unsafe {
        warn!("TARE_OFFSET before check: {}", TARE_OFFSET);

        if TARE_OFFSET == 0 {
            scale.tare(32);

            TARE_OFFSET = scale.get_offset();
        } else {
            scale.set_offset(TARE_OFFSET);
        }

        warn!("TARE_OFFSET after check: {}", TARE_OFFSET);
    }

    let mut wifi = Wifi::new(peripherals.modem)?;

    loop {
        wifi.connect(WIFI_SSID, WIFI_PASSWORD)?;

        let headers = [
            ("apikey", SUPABASE_KEY),
            ("Authorization", &format!("Bearer {}", SUPABASE_KEY)),
            ("Content-Type", "application/json"),
            ("Prefer", "return=representation"),
        ];

        let mut http = Http::new(&SUPABASE_URL, &headers)?;
        let payload_bytes = read_scale(&mut scale)?;

        http.post(&payload_bytes)?;
        wifi.disconnect()?;

        unsafe {
            info!("Will sleep for 30 minutes...");
            esp_idf_sys::esp_deep_sleep(Duration::from_secs(1800).as_micros() as u64);
        }
    }
}

fn read_scale(scale: &mut Scale<'_, Gpio3, Gpio2>) -> anyhow::Result<Vec<u8>> {
    let mut readings = Vec::new();

    loop {
        log::info!("Scale: starting...");

        if scale.is_ready() {
            log::info!("Scale: success!");

            let rounded_reading = scale.read_rounded().unwrap();
            let message = format!("Weight: {} g", rounded_reading);

            log::info!("{}", message);

            let payload = serde_json::json!({ "content": message });
            let payload_str = serde_json::to_string(&payload)?;
            let mut payload_bytes = payload_str.into_bytes();

            payload_bytes.push(b'\n');
            readings.extend(payload_bytes);

            break;
        }
    }

    Ok(readings)
}
