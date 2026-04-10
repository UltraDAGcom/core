//! WiFi station-mode connection helper.
//!
//! Blocks until the ESP32 has associated with the AP and acquired a DHCP
//! lease. Returns the `BlockingWifi<EspWifi>` handle — the caller must keep
//! it alive for the lifetime of the program, because dropping it tears the
//! WiFi stack down.

use anyhow::{Context, Result};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::modem::Modem;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};

/// Connect to the configured WiFi network and wait for a DHCP lease.
///
/// Returns a blocking handle that the caller owns. The function takes
/// ownership of the modem peripheral so only one WiFi session can exist.
pub fn connect(
    modem: Modem,
    sysloop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    ssid: &str,
    password: &str,
) -> Result<BlockingWifi<EspWifi<'static>>> {
    log::info!("WiFi: initializing station");

    let esp_wifi = EspWifi::new(modem, sysloop.clone(), Some(nvs))
        .context("EspWifi::new failed")?;
    let mut wifi = BlockingWifi::wrap(esp_wifi, sysloop).context("BlockingWifi::wrap failed")?;

    // WPA2-PSK is the only mode we support — pass `AuthMethod::None` if you
    // ever need to hit an open network (unauthenticated).
    let auth = if password.is_empty() {
        AuthMethod::None
    } else {
        AuthMethod::WPA2Personal
    };

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid
            .try_into()
            .map_err(|_| anyhow::anyhow!("WiFi SSID longer than 32 bytes"))?,
        password: password
            .try_into()
            .map_err(|_| anyhow::anyhow!("WiFi password longer than 64 bytes"))?,
        auth_method: auth,
        ..Default::default()
    }))
    .context("set_configuration failed")?;

    wifi.start().context("wifi.start failed")?;
    log::info!("WiFi: radio started, associating with '{}'…", ssid);

    wifi.connect().context("wifi.connect failed — wrong password or AP not found?")?;
    log::info!("WiFi: associated, waiting for DHCP lease…");

    wifi.wait_netif_up().context("wait_netif_up timed out — no DHCP reply?")?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info().context("get_ip_info failed")?;
    log::info!("WiFi: up. ip={} netmask={} gw={}",
        ip_info.ip, ip_info.subnet.mask, ip_info.subnet.gateway);

    Ok(wifi)
}
