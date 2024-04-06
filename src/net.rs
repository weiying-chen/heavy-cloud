use anyhow::Result;
use embedded_svc::http::client::Client as HttpClient;
use embedded_svc::io::Write;
use embedded_svc::utils::io;
use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};
use esp_idf_hal::modem::Modem;
use esp_idf_hal::sys::esp_wifi_set_max_tx_power;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use log::info;

pub struct Wifi<'a> {
    wifi: BlockingWifi<EspWifi<'a>>,
}

impl<'a> Wifi<'a> {
    pub fn new(modem: Modem) -> Result<Self> {
        let sys_loop = EspSystemEventLoop::take()?;
        let nvs = EspDefaultNvsPartition::take()?;

        let wifi = BlockingWifi::wrap(
            EspWifi::new(modem, sys_loop.clone(), Some(nvs))?,
            sys_loop.clone(),
        )?;

        Ok(Self { wifi })
    }

    pub fn connect(&mut self, ssid: &str, password: &str) -> Result<()> {
        let wifi_configuration = Configuration::Client(ClientConfiguration {
            ssid: ssid.try_into().unwrap(),
            bssid: None,
            auth_method: AuthMethod::WPA2Personal,
            password: password.try_into().unwrap(),
            channel: None,
        });

        self.wifi.set_configuration(&wifi_configuration)?;
        self.wifi.start()?;
        info!("Wifi started");

        unsafe { esp_wifi_set_max_tx_power(34) };

        self.wifi.connect()?;
        info!("Wifi connected");
        self.wifi.wait_netif_up()?;
        info!("Wifi netif up");

        Ok(())
    }
}

pub fn create_http_client() -> Result<HttpClient<EspHttpConnection>> {
    let config = &HttpConfiguration {
        buffer_size: Some(1024),
        buffer_size_tx: Some(1024),
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        ..Default::default()
    };

    let client = HttpClient::wrap(EspHttpConnection::new(&config)?);

    Ok(client)
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
