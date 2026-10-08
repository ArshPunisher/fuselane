//! Network interface discovery for Fuselane.
//!
//! Lists usable interfaces with a kind guess, and filters virtual, tunnel and
//! link-local-only adapters (L-60, L-102). Friendly names from native APIs
//! (SystemConfiguration, GetAdaptersAddresses, NetworkManager) and change
//! notifications come next (NETWORKING.md §1).

use std::collections::BTreeMap;
use std::net::IpAddr;

/// What kind of link an interface probably is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Wifi,
    Ethernet,
    Tether,
    Cellular,
    Vpn,
    Virtual,
    Loopback,
    Other,
}

/// One interface with its usable addresses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// OS device name (`en1`, `wlp2s0`, `Wi-Fi`).
    pub name: String,
    /// What the OS shows people ("Wi-Fi", "iPhone USB"); the device name when unknown.
    pub display_name: String,
    /// OS interface index (needed for pinning on macOS and Windows).
    pub index: u32,
    pub kind: Kind,
    /// Usable unicast addresses: no link-local, no loopback.
    pub addrs: Vec<IpAddr>,
}

impl Interface {
    /// Worth offering to the user: real kind and at least one usable address.
    pub fn usable(&self) -> bool {
        !self.addrs.is_empty() && !matches!(self.kind, Kind::Loopback | Kind::Virtual | Kind::Vpn)
    }
}

/// Link-local and other addresses that can't reach the internet (L-60).
pub fn is_usable_addr(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_multicast())
        }
        IpAddr::V6(v6) => {
            let seg = v6.segments()[0];
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (seg & 0xffc0) == 0xfe80)
        }
    }
}

/// Guesses the kind from the device name. Native APIs refine this per OS later;
/// names alone are enough to filter the adapters that must never be offered.
pub fn classify(name: &str) -> Kind {
    let n = name.to_ascii_lowercase();
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| n.starts_with(p));
    if (n.starts_with("lo") && n.len() <= 3) || n.contains("loopback") {
        return Kind::Loopback;
    }
    // Tunnels, VPNs, and Apple's peer-to-peer links (these can hang pinned connects, L-102).
    if starts(&[
        "utun", "tun", "tap", "wg", "zt", "ppp", "ipsec", "gif", "stf",
    ]) || n.contains("vpn")
        || n.contains("tailscale")
        || n.contains("wireguard")
    {
        return Kind::Vpn;
    }
    if starts(&[
        "awdl",
        "llw",
        "anpi",
        "ap",
        "bridge",
        "br-",
        "virbr",
        "docker",
        "veth",
        "vmnet",
        "vboxnet",
        "vethernet",
        "nan",
        "p2p",
        "lxc",
        "cni",
        "flannel",
        "kube",
    ]) || n.contains("hyper-v")
        || n.contains("virtualbox")
        || n.contains("vmware")
        || n.contains("wsl")
    {
        return Kind::Virtual;
    }
    if starts(&["wl", "wifi", "wi-fi", "wlan"]) || n.contains("wireless") {
        return Kind::Wifi;
    }
    if starts(&["usb", "enx", "rndis"])
        || n.contains("iphone")
        || n.contains("android")
        || n.contains("tether")
    {
        return Kind::Tether;
    }
    if starts(&["wwan", "rmnet", "ccmni", "pdp_ip"])
        || n.contains("cellular")
        || n.contains("mobile broadband")
    {
        return Kind::Cellular;
    }
    if starts(&["en", "eth", "eno", "enp", "ens"]) || n.contains("ethernet") {
        return Kind::Ethernet; // macOS en* is refined to Wi-Fi/Ethernet by SystemConfiguration later
    }
    Kind::Other
}

/// The OS index of an interface name (0 when unknown).
pub fn index_of(name: &str) -> u32 {
    let Ok(c) = std::ffi::CString::new(name) else {
        return 0;
    };
    // SAFETY: if_nametoindex reads a NUL-terminated string and returns 0 if unknown.
    #[cfg(unix)]
    let index = unsafe { libc::if_nametoindex(c.as_ptr()) };
    #[cfg(windows)]
    let index = unsafe {
        windows_sys::Win32::NetworkManagement::IpHelper::if_nametoindex(c.as_ptr().cast())
    };
    index
}

/// macOS: names and kinds from SystemConfiguration (L-61: never parse localized CLI output).
#[cfg(target_os = "macos")]
mod macos {
    use super::Kind;
    use std::collections::HashMap;
    use system_configuration::network_configuration::{
        SCNetworkInterfaceType as T, get_interfaces,
    };

    /// Maps a SystemConfiguration type (and display name) to our kind.
    pub fn kind(t: Option<&T>, display: &str) -> Option<Kind> {
        let d = display.to_ascii_lowercase();
        if d.contains("iphone")
            || d.contains("ipad")
            || d.contains("android")
            || d.contains("rndis")
        {
            return Some(Kind::Tether);
        }
        Some(match t? {
            T::IEEE80211 => Kind::Wifi,
            T::Ethernet | T::Bond | T::VLAN | T::FireWire => Kind::Ethernet,
            T::WWAN | T::Modem => Kind::Cellular,
            T::Bluetooth => Kind::Tether,
            T::Bridge => Kind::Virtual,
            T::PPP | T::PPTP | T::L2TP | T::IPSec | T::SixToFour => Kind::Vpn,
            T::Serial | T::IrDA | T::IPv4 => Kind::Other,
        })
    }

    /// BSD name → (display name, kind) for every interface SystemConfiguration knows.
    pub fn names() -> HashMap<String, (String, Option<Kind>)> {
        get_interfaces()
            .iter()
            .filter_map(|i| {
                let bsd = i.bsd_name()?.to_string();
                let display = i
                    .display_name()
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| bsd.clone());
                let k = kind(i.interface_type().as_ref(), &display);
                Some((bsd, (display, k)))
            })
            .collect()
    }
}

/// All interfaces with at least one address, usable addresses only.
pub fn list() -> std::io::Result<Vec<Interface>> {
    #[cfg(target_os = "macos")]
    let native = macos::names();
    let mut by_name: BTreeMap<String, Interface> = BTreeMap::new();
    for a in if_addrs::get_if_addrs()? {
        let entry = by_name.entry(a.name.clone()).or_insert_with(|| {
            #[allow(unused_mut)]
            let (mut display, mut kind) = (a.name.clone(), classify(&a.name));
            #[cfg(target_os = "macos")]
            if let Some((d, k)) = native.get(&a.name) {
                display = d.clone();
                // Tunnels and peer-to-peer links stay filtered whatever SC calls them (L-102).
                if !matches!(kind, Kind::Vpn | Kind::Virtual | Kind::Loopback) {
                    kind = k.unwrap_or(kind);
                }
            }
            Interface {
                index: a.index.unwrap_or_else(|| index_of(&a.name)),
                kind,
                name: a.name.clone(),
                display_name: display,
                addrs: vec![],
            }
        });
        let ip = a.ip();
        if is_usable_addr(&ip) && !entry.addrs.contains(&ip) {
            entry.addrs.push(ip);
        }
    }
    Ok(by_name.into_values().collect())
}

/// Interfaces worth offering for a transfer.
pub fn usable() -> std::io::Result<Vec<Interface>> {
    Ok(list()?.into_iter().filter(Interface::usable).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn classification_filters_what_must_never_be_offered() {
        for (name, kind) in [
            ("lo0", Kind::Loopback),
            ("lo", Kind::Loopback),
            ("utun3", Kind::Vpn),
            ("awdl0", Kind::Virtual),
            ("llw0", Kind::Virtual),
            ("anpi1", Kind::Virtual),
            ("bridge0", Kind::Virtual),
            ("docker0", Kind::Virtual),
            ("vEthernet (WSL)", Kind::Virtual),
            ("vmnet8", Kind::Virtual),
            ("wg0", Kind::Vpn),
            ("tailscale0", Kind::Vpn),
            ("wlp2s0", Kind::Wifi),
            ("Wi-Fi", Kind::Wifi),
            ("enx3a5b0c1d2e4f", Kind::Tether),
            ("usb0", Kind::Tether),
            ("enp3s0", Kind::Ethernet),
            ("Ethernet 2", Kind::Ethernet),
            ("en1", Kind::Ethernet),
            ("rmnet_data0", Kind::Cellular),
        ] {
            assert_eq!(classify(name), kind, "{name}");
        }
    }

    #[test]
    fn link_local_and_special_addresses_are_unusable() {
        let bad: [IpAddr; 6] = [
            Ipv4Addr::new(169, 254, 1, 2).into(),
            Ipv4Addr::LOCALHOST.into(),
            Ipv4Addr::UNSPECIFIED.into(),
            "fe80::1".parse::<Ipv6Addr>().unwrap().into(),
            Ipv6Addr::LOCALHOST.into(),
            "ff02::1".parse::<Ipv6Addr>().unwrap().into(),
        ];
        assert!(bad.iter().all(|ip| !is_usable_addr(ip)));
        let good: [IpAddr; 3] = [
            Ipv4Addr::new(192, 168, 1, 70).into(),
            Ipv4Addr::new(10, 0, 0, 2).into(),
            "2404:7c80::1".parse::<Ipv6Addr>().unwrap().into(),
        ];
        assert!(good.iter().all(is_usable_addr));
    }

    #[test]
    fn listing_works_and_never_offers_loopback_or_tunnels() {
        let all = list().unwrap();
        assert!(!all.is_empty(), "every machine has at least loopback");
        for i in usable().unwrap() {
            assert!(
                !matches!(i.kind, Kind::Loopback | Kind::Virtual | Kind::Vpn),
                "{i:?}"
            );
            assert!(!i.addrs.is_empty());
            assert!(i.addrs.iter().all(is_usable_addr));
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_kinds_come_from_system_configuration() {
        use system_configuration::network_configuration::SCNetworkInterfaceType as T;
        assert_eq!(macos::kind(Some(&T::IEEE80211), "Wi-Fi"), Some(Kind::Wifi));
        assert_eq!(
            macos::kind(Some(&T::Ethernet), "iPhone USB"),
            Some(Kind::Tether),
            "iPhone tethering shows as Ethernet"
        );
        assert_eq!(
            macos::kind(Some(&T::Ethernet), "USB 10/100/1000 LAN"),
            Some(Kind::Ethernet)
        );
        assert_eq!(
            macos::kind(Some(&T::WWAN), "Cellular"),
            Some(Kind::Cellular)
        );
        assert_eq!(
            macos::kind(Some(&T::Bridge), "Thunderbolt Bridge"),
            Some(Kind::Virtual)
        );
        assert_eq!(macos::kind(None, "Something"), None);
        // On a real Mac every usable interface gets a display name, and Wi-Fi isn't called Ethernet.
        for i in usable().unwrap() {
            assert!(!i.display_name.is_empty());
            if i.display_name.to_lowercase().contains("wi-fi") {
                assert_eq!(i.kind, Kind::Wifi, "{i:?}");
            }
        }
    }

    #[test]
    fn index_lookup() {
        assert_eq!(index_of("definitely-not-an-interface"), 0);
        assert_eq!(index_of("bad\0name"), 0);
        #[cfg(unix)]
        assert!(
            index_of(if cfg!(target_os = "linux") {
                "lo"
            } else {
                "lo0"
            }) > 0
        );
    }
}
