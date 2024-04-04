use embedded_svc::{
    http::client::Client as HttpClient,
    io::Write,
    utils::io,
    wifi::{AuthMethod, ClientConfiguration, Configuration},
};
use esp_idf_hal::prelude::Peripherals;
use esp_idf_hal::sys::esp_wifi_set_max_tx_power;
use esp_idf_svc::hal::delay::FreeRtos;
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};
use esp_idf_svc::log::EspLogger;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use esp_idf_svc::{eventloop::EspSystemEventLoop, nvs::EspDefaultNvsPartition};
use esp_idf_sys::{self as _}; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use log::{error, info};

mod critical_section;
mod net;
mod scale;
use crate::scale::Scale;

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_PASSWORD: &str = env!("WIFI_PASS");
const SUPABASE_KEY: &str = env!("SUPABASE_KEY");
const SUPABASE_URL: &str = env!("SUPABASE_URL");
const LOAD_SENSOR_SCALING: f32 = 0.0027;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    net::connect_wifi(&mut wifi, WIFI_SSID, WIFI_PASSWORD)?;

    let config = &HttpConfiguration {
        buffer_size: Some(1024),
        buffer_size_tx: Some(1024),
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        ..Default::default()
    };

    let mut client = HttpClient::wrap(EspHttpConnection::new(&config)?);
    let dt = peripherals.pins.gpio2;
    let sck = peripherals.pins.gpio3;
    let mut scale = Scale::new(sck, dt, LOAD_SENSOR_SCALING).unwrap();

    scale.tare(32);

    let mut iterations = 0;

    loop {
        log::info!("Waiting for the scale to be ready...");
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

            net::post_request(&mut client, payload_bytes, SUPABASE_KEY, SUPABASE_URL)?;
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
