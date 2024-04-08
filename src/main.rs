use esp_idf_hal::gpio::{Gpio2, Gpio3};
use esp_idf_hal::prelude::Peripherals;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::log::EspLogger;
use esp_idf_sys::{self as _}; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use log::info;

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

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;

    // init scale

    let dt = peripherals.pins.gpio2;
    let sck = peripherals.pins.gpio3;
    let mut scale = Scale::new(sck, dt, LOAD_SENSOR_SCALING).unwrap();

    scale.tare(32);

    // init wifi

    let mut wifi = Wifi::new(peripherals.modem)?;

    loop {
        log::info!("Wifi: starting...");

        wifi.connect(WIFI_SSID, WIFI_PASSWORD)?;

        log::info!("Wifi: success!");

        // // initi http

        // let headers = [
        //     ("apikey", SUPABASE_KEY),
        //     ("Authorization", &format!("Bearer {}", SUPABASE_KEY)),
        //     ("Content-Type", "application/json"),
        //     ("Prefer", "return=representation"),
        //     // ("Content-Length", &content_length_header),
        // ];

        // let mut http = Http::new(&SUPABASE_URL, &headers)?;
        // // let mut iterations = 0;

        // let payload_bytes = collect_readings(&mut scale)?;

        // http.post(&payload_bytes)?;

        info!("Shutting down in 5s...");

        wifi.disconnect()?;

        // info!("Wifi disconnected!");

        FreeRtos::delay_ms(10000u32);
    }

    Ok(())
}

fn collect_readings(scale: &mut Scale<'_, Gpio3, Gpio2>) -> anyhow::Result<Vec<u8>> {
    let mut readings = Vec::new();

    loop {
        log::info!("Scale: starting...");

        if scale.is_ready() {
            log::info!("Scale: success!");

            // Read scale and create message
            let rounded_reading = scale.read_rounded().unwrap();
            let message = format!("Weight: {} g", rounded_reading);
            log::info!("{}", message);

            // Serialize message to JSON
            let payload = serde_json::json!({ "content": message });
            let payload_str = serde_json::to_string(&payload)?;
            let mut payload_bytes = payload_str.into_bytes();

            // Add a newline as a delimiter for each reading
            payload_bytes.push(b'\n');

            // Concatenate this reading's bytes to the overall readings vector
            readings.extend(payload_bytes);

            break; // Remove this if you want to collect multiple readings
        }
    }

    Ok(readings)
}
