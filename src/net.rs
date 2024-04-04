use anyhow::Result;
use embedded_svc::http::client::Client as HttpClient;
use embedded_svc::io::Write;
use embedded_svc::utils::io;
use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};
use esp_idf_hal::sys::esp_wifi_set_max_tx_power;
use esp_idf_svc::http::client::EspHttpConnection;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use log::info;

pub fn connect_wifi(
    wifi: &mut BlockingWifi<EspWifi<'static>>,
    wifi_ssid: &str,
    wifi_password: &str,
) -> Result<()> {
    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: wifi_ssid.try_into().unwrap(),
        bssid: None,
        auth_method: AuthMethod::WPA2Personal,
        password: wifi_password.try_into().unwrap(),
        channel: None,
    });

    wifi.set_configuration(&wifi_configuration)?;
    wifi.start()?;
    info!("Wifi started");

    unsafe { esp_wifi_set_max_tx_power(34) };

    wifi.connect()?;
    info!("Wifi connected");
    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    Ok(())
}

pub fn post_request(
    client: &mut HttpClient<EspHttpConnection>,
    payload: &[u8],
    supabase_key: &str,
    supabase_url: &str,
) -> Result<()> {
    let content_length_header = format!("{}", payload.len());

    let headers = [
        ("apikey", supabase_key),
        ("Authorization", &format!("Bearer {}", supabase_key)),
        ("Content-Type", "application/json"),
        ("Prefer", "return=representation"),
        ("Content-Length", &content_length_header),
    ];

    let mut request = client.post(supabase_url, &headers)?;

    request.write_all(payload)?;
    request.flush()?;

    info!("-> POST {}", supabase_url);

    let mut response = request.submit()?;
    let status = response.status();

    info!("<- {}", status);

    let mut buf = [0u8; 1024];
    let bytes_read = io::try_read_full(&mut response, &mut buf).map_err(|e| e.0)?;

    info!("Read {} bytes", bytes_read);

    while response.read(&mut buf)? > 0 {}

    Ok(())
}
