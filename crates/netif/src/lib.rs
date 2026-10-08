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

/// What Linux publishes about an interface under /sys/class/net/<name>.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SysFacts {
    /// `wireless/` or `phy80211` exists.
    pub wireless: bool,
    /// The driver behind `device/driver` (for example `rndis_host`, `ipheth`).
    pub driver: Option<String>,
    /// The device lives under /sys/devices/virtual (bridges, veth, docker).
    pub virtual_device: bool,
    /// `tun_flags` exists (TUN/TAP: VPNs).
    pub tun: bool,
    /// ARPHRD type from `type` (1 = Ethernet, 772 = loopback, 65534 = none).
    pub arp_type: Option<u32>,
}

/// Linux: kind and friendly name from sysfs facts; `None` keeps the name guess.
pub fn linux_kind(f: &SysFacts) -> Option<(Kind, &'static str)> {
    if f.arp_type == Some(772) {
        return Some((Kind::Loopback, "Loopback"));
    }
    if f.tun || f.arp_type == Some(65534) {
        return Some((Kind::Vpn, "VPN"));
    }
    match f.driver.as_deref() {
        Some("ipheth") => return Some((Kind::Tether, "iPhone USB")),
        Some("rndis_host" | "cdc_ether" | "cdc_ncm" | "cdc_eem") => {
            return Some((Kind::Tether, "Phone USB"));
        }
        Some("qmi_wwan" | "cdc_mbim" | "option" | "huawei_cdc_ncm" | "sierra_net") => {
            return Some((Kind::Cellular, "Mobile broadband"));
        }
        _ => {}
    }
    if f.wireless {
        return Some((Kind::Wifi, "Wi-Fi"));
    }
    if f.virtual_device {
        return Some((Kind::Virtual, "Virtual"));
    }
    if f.arp_type == Some(1) && f.driver.is_some() {
        return Some((Kind::Ethernet, "Ethernet"));
    }
    None
}

#[cfg(target_os = "linux")]
fn sys_facts(name: &str) -> SysFacts {
    let base = std::path::Path::new("/sys/class/net").join(name);
    let driver = std::fs::read_link(base.join("device/driver"))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
    let real = std::fs::canonicalize(&base).unwrap_or_default();
    SysFacts {
        wireless: base.join("wireless").exists() || base.join("phy80211").exists(),
        driver,
        virtual_device: real.starts_with("/sys/devices/virtual"),
        tun: base.join("tun_flags").exists(),
        arp_type: std::fs::read_to_string(base.join("type"))
            .ok()
            .and_then(|s| s.trim().parse().ok()),
    }
}

/// Windows: kind from the adapter's IANA ifType and description; the friendly
/// name ("Wi-Fi", "Ethernet 2") is already what people see in Settings.
pub fn windows_kind(if_type: u32, description: &str) -> Option<Kind> {
    let d = description.to_ascii_lowercase();
    if d.contains("apple mobile device")
        || d.contains("remote ndis")
        || d.contains("rndis")
        || d.contains("android")
    {
        return Some(Kind::Tether);
    }
    // Inside a VM (Hyper-V, Azure, Parallels on Windows hosts), "Microsoft Hyper-V
    // Network Adapter" is the machine's real network card. The host's virtual
    // switch is the "Hyper-V Virtual Ethernet Adapter" (vEthernet), skipped below.
    if d.contains("hyper-v network adapter") {
        return Some(Kind::Ethernet);
    }
    if d.contains("hyper-v")
        || d.contains("virtualbox")
        || d.contains("vmware")
        || d.contains("wsl")
        || d.contains("loopback")
    {
        return Some(if d.contains("loopback") {
            Kind::Loopback
        } else {
            Kind::Virtual
        });
    }
    if d.contains("wireguard")
        || d.contains("tap-windows")
        || d.contains("wintun")
        || d.contains("vpn")
        || d.contains("tailscale")
    {
        return Some(Kind::Vpn);
    }
    Some(match if_type {
        71 => Kind::Wifi,            // IF_TYPE_IEEE80211
        6 => Kind::Ethernet,         // IF_TYPE_ETHERNET_CSMACD
        243 | 244 => Kind::Cellular, // WWANPP, WWANPP2
        24 => Kind::Loopback,        // SOFTWARE_LOOPBACK
        53 | 131 => Kind::Vpn,       // PROP_VIRTUAL, TUNNEL
        _ => return None,
    })
}

/// Windows: ifIndex → (friendly name, kind) from GetAdaptersAddresses.
#[cfg(windows)]
mod windows_names {
    use super::{Kind, windows_kind};
    use std::collections::HashMap;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
        IP_ADAPTER_ADDRESSES_LH,
    };

    fn wide(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        // SAFETY: Windows returns NUL-terminated UTF-16 strings that live as long as the buffer.
        unsafe {
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
        }
    }

    pub fn names() -> HashMap<u32, (String, Option<Kind>)> {
        let mut out = HashMap::new();
        let mut size: u32 = 16 * 1024;
        for _ in 0..3 {
            let mut buf = vec![0u8; size as usize];
            let first = buf.as_mut_ptr().cast::<IP_ADAPTER_ADDRESSES_LH>();
            // SAFETY: the buffer is `size` bytes; Windows fills a linked list inside it.
            let rc = unsafe {
                GetAdaptersAddresses(
                    0,
                    GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST,
                    std::ptr::null(),
                    first,
                    &mut size,
                )
            };
            if rc == 111 {
                continue; // ERROR_BUFFER_OVERFLOW: `size` now holds what's needed
            }
            if rc != 0 {
                return out;
            }
            let mut cur = first.cast_const();
            while !cur.is_null() {
                // SAFETY: walking the list Windows just wrote into `buf`.
                let a = unsafe { &*cur };
                // SAFETY: IfIndex is the first field of the Anonymous1 union's struct.
                let index = unsafe { a.Anonymous1.Anonymous.IfIndex };
                let friendly = wide(a.FriendlyName);
                let desc = wide(a.Description);
                out.insert(index, (friendly, windows_kind(a.IfType, &desc)));
                cur = a.Next;
            }
            return out;
        }
        out
    }
}

/// All interfaces with at least one address, usable addresses only.
pub fn list() -> std::io::Result<Vec<Interface>> {
    #[cfg(target_os = "macos")]
    let native = macos::names();
    #[cfg(windows)]
    let win = windows_names::names();
    let mut by_name: BTreeMap<String, Interface> = BTreeMap::new();
    for a in if_addrs::get_if_addrs()? {
        let entry = by_name.entry(a.name.clone()).or_insert_with(|| {
            #[allow(unused_mut)]
            let (mut display, mut kind) = (a.name.clone(), classify(&a.name));
            #[cfg(target_os = "linux")]
            if !matches!(kind, Kind::Vpn | Kind::Virtual | Kind::Loopback)
                && let Some((k, d)) = linux_kind(&sys_facts(&a.name))
            {
                kind = k;
                display = d.to_string();
            }
            #[cfg(windows)]
            if let Some((d, k)) = a.index.and_then(|i| win.get(&i)) {
                if !d.is_empty() {
                    display = d.clone();
                }
                if let Some(k) = k {
                    kind = *k;
                }
            }
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
    fn linux_sysfs_facts_name_and_classify_devices() {
        let f = |wireless, driver: Option<&str>, virtual_device, tun, arp| SysFacts {
            wireless,
            driver: driver.map(Into::into),
            virtual_device,
            tun,
            arp_type: arp,
        };
        assert_eq!(
            linux_kind(&f(true, Some("iwlwifi"), false, false, Some(1))),
            Some((Kind::Wifi, "Wi-Fi"))
        );
        assert_eq!(
            linux_kind(&f(false, Some("e1000e"), false, false, Some(1))),
            Some((Kind::Ethernet, "Ethernet"))
        );
        assert_eq!(
            linux_kind(&f(false, Some("ipheth"), false, false, Some(1))),
            Some((Kind::Tether, "iPhone USB"))
        );
        assert_eq!(
            linux_kind(&f(false, Some("rndis_host"), false, false, Some(1))),
            Some((Kind::Tether, "Phone USB"))
        );
        assert_eq!(
            linux_kind(&f(false, Some("qmi_wwan"), false, false, Some(1))),
            Some((Kind::Cellular, "Mobile broadband"))
        );
        assert_eq!(
            linux_kind(&f(false, None, true, false, Some(1))),
            Some((Kind::Virtual, "Virtual")),
            "docker0, veth"
        );
        assert_eq!(
            linux_kind(&f(false, None, true, true, Some(65534))),
            Some((Kind::Vpn, "VPN")),
            "tun0"
        );
        assert_eq!(
            linux_kind(&f(false, None, true, false, Some(772))),
            Some((Kind::Loopback, "Loopback"))
        );
        assert_eq!(
            linux_kind(&f(false, None, false, false, None)),
            None,
            "unknown keeps the name guess"
        );
    }

    #[test]
    fn windows_adapters_are_classified_by_type_and_description() {
        assert_eq!(
            windows_kind(71, "Intel(R) Wi-Fi 6 AX201 160MHz"),
            Some(Kind::Wifi)
        );
        assert_eq!(
            windows_kind(6, "Realtek PCIe GbE Family Controller"),
            Some(Kind::Ethernet)
        );
        assert_eq!(
            windows_kind(6, "Apple Mobile Device Ethernet"),
            Some(Kind::Tether)
        );
        assert_eq!(
            windows_kind(6, "Remote NDIS based Internet Sharing Device"),
            Some(Kind::Tether)
        );
        assert_eq!(
            windows_kind(6, "Hyper-V Virtual Ethernet Adapter"),
            Some(Kind::Virtual)
        );
        // A Windows guest's own card (GitHub's runners, Azure VMs) is real.
        assert_eq!(
            windows_kind(6, "Microsoft Hyper-V Network Adapter"),
            Some(Kind::Ethernet)
        );
        assert_eq!(
            windows_kind(6, "Microsoft Hyper-V Network Adapter #2"),
            Some(Kind::Ethernet)
        );
        assert_eq!(windows_kind(53, "WireGuard Tunnel"), Some(Kind::Vpn));
        assert_eq!(
            windows_kind(243, "Generic Mobile Broadband Adapter"),
            Some(Kind::Cellular)
        );
        assert_eq!(
            windows_kind(24, "Software Loopback Interface 1"),
            Some(Kind::Loopback)
        );
        assert_eq!(windows_kind(9999, "Something new"), None);
    }

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
