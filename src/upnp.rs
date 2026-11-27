//! UPnP IGD: asks the home router to forward a TCP port to this PC, so
//! friends on the internet can reach the game server without manual
//! router setup. Plain SSDP + SOAP, no extra crates.

use anyhow::{anyhow, bail, Context, Result};
use std::net::{Ipv4Addr, UdpSocket};
use std::time::{Duration, Instant};

const SSDP_ADDR: &str = "239.255.255.250:1900";
const DESCRIPTION: &str = "Sad Launcher";

/// A router service that can map ports.
#[derive(Clone, Debug)]
pub struct Gateway {
    pub control_url: String,
    pub service_type: String,
}

/// What opening a port achieved.
#[derive(Clone, Debug)]
pub struct Opened {
    /// WAN address the router reports (may be private behind CGNAT).
    pub external_ip: Option<String>,
}

/// Private or carrier-grade NAT address: the internet cannot reach it.
pub fn is_unreachable_ip(ip: &str) -> bool {
    let Ok(addr) = ip.trim().parse::<Ipv4Addr>() else {
        return false;
    };
    let [a, b, ..] = addr.octets();
    addr.is_private() || addr.is_loopback() || addr.is_link_local() || (a == 100 && (64..128).contains(&b))
}

/// Home-network address among the default routes (gateway, interface,
/// metric): the interface whose default route goes through a real gateway
/// in a private range. VPN/TUN adapters (gateway 0.0.0.0 / on-link) and
/// virtual switches are skipped; the lowest metric wins.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn pick_home_ip(routes: &[(Ipv4Addr, Ipv4Addr, u32)]) -> Option<String> {
    routes
        .iter()
        .filter(|(gw, iface, _)| gw.is_private() && !gw.is_unspecified() && iface.is_private())
        .min_by_key(|(_, _, metric)| *metric)
        .map(|(_, iface, _)| iface.to_string())
}

/// This PC's address in the home network (what friends on the same Wi-Fi
/// use and what the router forwards to). Read from the IPv4 route table
/// directly — `route print` output is localized and costs a process.
#[cfg(windows)]
pub fn home_ip() -> Option<String> {
    pick_home_ip(&crate::win::default_routes())
}

/// Linux/macOS: the address the default route uses, when it is a private
/// one (`route print` exists only on Windows).
#[cfg(unix)]
pub fn home_ip() -> Option<String> {
    default_route_ip().filter(|ip| ip.parse::<Ipv4Addr>().map(|a| a.is_private()).unwrap_or(false))
}

/// Address the default route leaves through — differs from `home_ip`
/// when a VPN captures all traffic.
pub fn default_route_ip() -> Option<String> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    Some(sock.local_addr().ok()?.ip().to_string())
}

/// The real home internet address: asked through the home interface, so a
/// VPN does not substitute its own exit address.
pub async fn public_ip(home: Option<String>) -> Option<String> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(10));
    if let Some(ip) = home.as_deref().and_then(|s| s.parse::<std::net::IpAddr>().ok()) {
        builder = builder.local_address(ip);
    }
    let client = builder.build().ok()?;
    for url in ["https://api.ipify.org", "https://ipv4.icanhazip.com"] {
        if let Ok(r) = client.get(url).send().await {
            if let Ok(t) = r.text().await {
                let t = t.trim().to_string();
                if t.parse::<Ipv4Addr>().is_ok() {
                    return Some(t);
                }
            }
        }
    }
    None
}

/// Finds the router's port-mapping service via SSDP multicast.
fn discover(local_ip: Option<&str>) -> Result<Vec<String>> {
    let bind = format!("{}:0", local_ip.unwrap_or("0.0.0.0"));
    let sock = UdpSocket::bind(&bind).or_else(|_| UdpSocket::bind("0.0.0.0:0"))?;
    sock.set_read_timeout(Some(Duration::from_millis(400)))?;
    for st in [
        "urn:schemas-upnp-org:device:InternetGatewayDevice:1",
        "urn:schemas-upnp-org:device:InternetGatewayDevice:2",
        "urn:schemas-upnp-org:service:WANIPConnection:1",
    ] {
        let msg = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: {SSDP_ADDR}\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: {st}\r\n\r\n"
        );
        let _ = sock.send_to(msg.as_bytes(), SSDP_ADDR);
    }
    let mut found: Vec<String> = Vec::new();
    let mut buf = [0u8; 2048];
    let deadline = Instant::now() + Duration::from_millis(2500);
    while Instant::now() < deadline {
        if let Ok((n, _)) = sock.recv_from(&mut buf) {
            let text = String::from_utf8_lossy(&buf[..n]);
            for line in text.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    if k.trim().eq_ignore_ascii_case("location") {
                        let url = v.trim().to_string();
                        if !found.contains(&url) {
                            found.push(url);
                        }
                    }
                }
            }
        }
    }
    if found.is_empty() {
        bail!("роутер не ответил по UPnP");
    }
    Ok(found)
}

/// Text between `<tag>` and `</tag>` (namespace prefixes ignored).
fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let mut search = xml;
    loop {
        let start = search.find('<')?;
        let rest = &search[start + 1..];
        let end = rest.find('>')?;
        let head = &rest[..end];
        let local = head.split_whitespace().next().unwrap_or("");
        let local = local.trim_end_matches('/');
        let local = local.rsplit(':').next().unwrap_or(local);
        if local == name && !head.starts_with('/') {
            if head.trim_end().ends_with('/') {
                return Some("");
            }
            let body = &rest[end + 1..];
            let close = body.find("</")?;
            return Some(body[..close].trim());
        }
        search = &rest[end + 1..];
    }
}

/// Picks the WAN connection service out of a device description.
pub fn parse_description(location: &str, xml: &str) -> Option<Gateway> {
    let base = tag(xml, "URLBase")
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let after = location.split_once("://").unwrap_or(("http", location));
            let host = after.1.split('/').next().unwrap_or("");
            format!("{}://{}", after.0, host)
        });
    for chunk in xml.split("<service>").skip(1) {
        let Some(st) = tag(chunk, "serviceType") else { continue };
        if st.contains("WANIPConnection") || st.contains("WANPPPConnection") {
            let Some(ctrl) = tag(chunk, "controlURL") else { continue };
            let control_url = if ctrl.starts_with("http") {
                ctrl.to_string()
            } else {
                format!("{}/{}", base, ctrl.trim_start_matches('/'))
            };
            return Some(Gateway {
                control_url,
                service_type: st.to_string(),
            });
        }
    }
    None
}

async fn soap(client: &reqwest::Client, gw: &Gateway, action: &str, args: &str) -> Result<String> {
    let body = format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{st}\">{args}</u:{action}></s:Body></s:Envelope>",
        st = gw.service_type
    );
    let resp = client
        .post(&gw.control_url)
        .header("Content-Type", "text/xml; charset=\"utf-8\"")
        .header("SOAPAction", format!("\"{}#{}\"", gw.service_type, action))
        .timeout(Duration::from_secs(6))
        .body(body)
        .send()
        .await
        .context("роутер не ответил")?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let code = tag(&text, "errorCode").unwrap_or("?");
        let desc = tag(&text, "errorDescription").unwrap_or("");
        bail!("роутер отказал ({code} {desc})");
    }
    Ok(text)
}

async fn gateway(client: &reqwest::Client, local_ip: Option<String>) -> Result<Gateway> {
    let li = local_ip.clone();
    let locations = tokio::task::spawn_blocking(move || discover(li.as_deref()))
        .await
        .map_err(|_| anyhow!("поиск роутера прервался"))??;
    for loc in locations {
        let Ok(resp) = client.get(&loc).timeout(Duration::from_secs(5)).send().await else {
            continue;
        };
        let Ok(xml) = resp.text().await else { continue };
        if let Some(gw) = parse_description(&loc, &xml) {
            return Ok(gw);
        }
    }
    bail!("у роутера нет службы проброса портов")
}

/// Forwards TCP `port` on the router to this PC.
pub async fn open(client: &reqwest::Client, port: u16, local_ip: Option<String>) -> Result<Opened> {
    let lan = local_ip.clone().ok_or_else(|| anyhow!("не найден адрес ПК в локальной сети"))?;
    let gw = gateway(client, local_ip).await?;
    let args = |lease: u32| {
        format!(
            "<NewRemoteHost></NewRemoteHost><NewExternalPort>{port}</NewExternalPort><NewProtocol>TCP</NewProtocol><NewInternalPort>{port}</NewInternalPort><NewInternalClient>{lan}</NewInternalClient><NewEnabled>1</NewEnabled><NewPortMappingDescription>{DESCRIPTION}</NewPortMappingDescription><NewLeaseDuration>{lease}</NewLeaseDuration>"
        )
    };
    // Permanent first (closed again on exit); some routers only take leases.
    if soap(client, &gw, "AddPortMapping", &args(0)).await.is_err() {
        soap(client, &gw, "AddPortMapping", &args(86_400)).await?;
    }
    let external_ip = soap(client, &gw, "GetExternalIPAddress", "")
        .await
        .ok()
        .and_then(|xml| tag(&xml, "NewExternalIPAddress").map(String::from))
        .filter(|s| !s.is_empty());
    Ok(Opened { external_ip })
}

/// Removes the mapping made by `open`.
pub async fn close(client: &reqwest::Client, port: u16, local_ip: Option<String>) -> Result<()> {
    let gw = gateway(client, local_ip).await?;
    soap(
        client,
        &gw,
        "DeletePortMapping",
        &format!("<NewRemoteHost></NewRemoteHost><NewExternalPort>{port}</NewExternalPort><NewProtocol>TCP</NewProtocol>"),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_wan_service_and_joins_relative_url() {
        let xml = r#"<?xml version="1.0"?><root><device><serviceList>
            <service><serviceType>urn:schemas-upnp-org:service:Layer3Forwarding:1</serviceType><controlURL>/l3f</controlURL></service>
            </serviceList><deviceList><device><serviceList>
            <service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ctl/IPConn</controlURL></service>
            </serviceList></device></deviceList></device></root>"#;
        let gw = parse_description("http://192.168.1.1:5000/rootDesc.xml", xml).unwrap();
        assert_eq!(gw.control_url, "http://192.168.1.1:5000/ctl/IPConn");
        assert_eq!(gw.service_type, "urn:schemas-upnp-org:service:WANIPConnection:1");
    }

    #[test]
    fn reads_namespaced_soap_values() {
        let xml = "<s:Envelope><s:Body><u:GetExternalIPAddressResponse><NewExternalIPAddress>93.184.216.34</NewExternalIPAddress></u:GetExternalIPAddressResponse></s:Body></s:Envelope>";
        assert_eq!(tag(xml, "NewExternalIPAddress"), Some("93.184.216.34"));
    }

    #[test]
    fn route_table_picks_home_interface_not_vpn() {
        let ip = |s: &str| s.parse::<Ipv4Addr>().unwrap();
        // Home router + a VPN adapter whose «gateway» is on-link (0.0.0.0).
        let routes = [
            (ip("192.168.1.254"), ip("192.168.1.65"), 40),
            (ip("0.0.0.0"), ip("172.19.0.1"), 0),
        ];
        assert_eq!(pick_home_ip(&routes).as_deref(), Some("192.168.1.65"));
        assert_eq!(pick_home_ip(&[(ip("0.0.0.0"), ip("10.8.0.2"), 1)]), None);
    }

    #[test]
    fn cgnat_and_private_are_unreachable() {
        assert!(is_unreachable_ip("100.72.1.5"));
        assert!(is_unreachable_ip("192.168.0.10"));
        assert!(is_unreachable_ip("10.0.0.1"));
        assert!(!is_unreachable_ip("93.184.216.34"));
        assert!(!is_unreachable_ip("100.128.0.1"));
    }

    #[test]
    fn parses_description_with_self_closing_and_unrelated_service() {
        let xml = r#"
        <root>
            <URLBase/>
            <device>
                <serviceList>
                    <service>
                        <serviceType>urn:schemas-upnp-org:service:Layer3Forwarding:1</serviceType>
                    </service>
                    <service>
                        <serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType>
                        <controlURL>/ctl/IPConn</controlURL>
                    </service>
                </serviceList>
            </device>
        </root>"#;
        let gw = parse_description("http://192.168.1.1:5000/rootDesc.xml", xml).unwrap();
        assert_eq!(gw.control_url, "http://192.168.1.1:5000/ctl/IPConn");
    }
}

#[cfg(test)]
mod live {
    /// Read-only: finds the router's port-mapping service, maps nothing.
    /// `cargo test upnp::live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn router_answers_discovery() {
        let lan = crate::server::local_ip();
        let res = crate::net::rt().block_on(super::gateway(&crate::net::client(), lan.clone()));
        println!("lan={lan:?} gateway={res:?}");
    }
}
