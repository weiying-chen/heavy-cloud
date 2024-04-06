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
    let mut wifi = Wifi::new(peripherals.modem)?;

    wifi.connect(WIFI_SSID, WIFI_PASSWORD)?;

    let mut http = Http::new()?;
    let dt = peripherals.pins.gpio2;
    let sck = peripherals.pins.gpio3;
    let mut scale = Scale::new(sck, dt, LOAD_SENSOR_SCALING).unwrap();

    scale.tare(32);

    let mut iterations = 0;

    loop {
        log::info!("Preparing scale...");
        if scale.is_ready() {
            log::info!("Iteration {}", iterations);

            let rounded_reading = scale.read_rounded().unwrap();
            let message = format!("Weight: {} g", rounded_reading);

            log::info!("{}", message);

            let payload = serde_json::json!({
                "content": message
            });

            let payload_str = serde_json::to_string(&payload).unwrap();
            let payload_bytes = payload_str.as_bytes();

            http.post_supabase(payload_bytes, SUPABASE_KEY, SUPABASE_URL)?;
        }

        FreeRtos::delay_ms(5000u32);

        iterations += 1;

        if iterations >= 4 {
            break;
        }
    }

    info!("Shutting down in 5s...");

    std::thread::sleep(core::time::Duration::from_secs(5));

    Ok(())
}
