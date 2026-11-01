//! Translation keys. All visible UI strings go through [`t`].

use std::sync::atomic::{AtomicU8, Ordering};
use storage::Language;

static LANG: AtomicU8 = AtomicU8::new(0); // 0 = de, 1 = en

pub fn set_language(lang: Language) {
    let en = match lang {
        Language::German => false,
        Language::English => true,
        Language::System => !sys_locale::get_locale().unwrap_or_default().to_lowercase().starts_with("de"),
    };
    LANG.store(u8::from(en), Ordering::Relaxed);
}

pub fn is_english() -> bool {
    LANG.load(Ordering::Relaxed) == 1
}

/// (key, Deutsch, English)
const STRINGS: &[(&str, &str, &str)] = &[
    ("app.title", "IP Scanner Deluxe", "IP Scanner Deluxe"),
    // Menus
    ("menu.file", "Datei", "File"),
    ("menu.view", "Ansicht", "View"),
    ("menu.settings", "Einstellungen", "Settings"),
    ("menu.help", "Hilfe", "Help"),
    ("file.new", "Neue Suche", "New search"),
    ("file.load", "Liste laden...", "Load list..."),
    ("file.save_as", "Speichern als", "Save as"),
    ("file.quit", "Beenden", "Exit"),
    ("file.favorites_import", "Favoriten importieren (XML)...", "Import favorites (XML)..."),
    ("file.favorites_export", "Favoriten exportieren (XML)...", "Export favorites (XML)..."),
    ("view.expand_all", "Alles erweitern", "Expand all"),
    ("view.collapse_all", "Alles reduzieren", "Collapse all"),
    ("view.alive_only", "Nur aktive Geräte", "Alive devices only"),
    ("view.show_offline", "Offline-Geräte anzeigen", "Show offline devices"),
    ("view.show_unknown", "Unbekannte Geräte anzeigen", "Show unknown devices"),
    ("view.toolbar", "Toolbar anpassen", "Customize toolbar"),
    ("view.toolbar_ipc", "IP- und C-Schaltflächen", "IP and C buttons"),
    ("view.toolbar_expand", "Erweitern/Reduzieren", "Expand/Collapse"),
    ("view.columns", "Spalten auswählen", "Choose columns"),
    ("view.dark", "Dunkelmodus", "Dark mode"),
    ("view.theme_system", "Systemdesign verwenden", "Follow system theme"),
    ("view.alternating", "Alternierende Tabellenzeilen", "Alternating rows"),
    ("settings.options", "Optionen...", "Options..."),
    ("settings.macdb", "MAC-Datenbank", "MAC database"),
    ("settings.profiles", "Scan-Profile", "Scan profiles"),
    ("settings.fingerprints", "Fingerprints", "Fingerprints"),
    ("settings.language", "Sprache", "Language"),
    ("lang.system", "Systemsprache", "System language"),
    ("lang.de", "Deutsch", "Deutsch"),
    ("lang.en", "English", "English"),
    ("help.about", "Über IP Scanner Deluxe", "About IP Scanner Deluxe"),
    ("help.diagnose", "Diagnose", "Diagnostics"),
    ("help.log", "Logdatei öffnen", "Open log file"),
    // Toolbar
    ("tb.scan", "Scannen", "Scan"),
    ("tb.stop", "Stoppen", "Stop"),
    ("tb.pause", "Pause", "Pause"),
    ("tb.resume", "Fortsetzen", "Resume"),
    ("tb.ip", "Subnetz dieses Computers (IP)", "Subnet of this computer (IP)"),
    ("tb.c", "Klasse-C-Subnetz (C)", "Class C subnet (C)"),
    ("tb.expand", "Alles erweitern", "Expand all"),
    ("tb.collapse", "Alles reduzieren", "Collapse all"),
    ("range.hint", "Beispiel: 192.168.0.1-100, 192.168.0.200", "Example: 192.168.0.1-100, 192.168.0.200"),
    ("search.hint", "Suche", "Search"),
    ("tab.results", "Ergebnisliste", "Results"),
    ("tab.favorites", "Favoriten", "Favorites"),
    // Columns
    ("col.status", "Status", "Status"),
    ("col.name", "Name", "Name"),
    ("col.ip", "IP", "IP"),
    ("col.vendor", "Hersteller", "Manufacturer"),
    ("col.mac", "MAC-Adresse", "MAC address"),
    ("col.comment", "Kommentar", "Comments"),
    ("col.device_type", "Gerätetyp", "Device type"),
    ("col.model", "Modell", "Model"),
    ("col.os", "Betriebssystem", "Operating system"),
    ("col.latency", "Antwortzeit", "Response time"),
    ("col.services", "Offene Dienste", "Open services"),
    ("col.confidence", "Erkennung", "Detection"),
    ("col.last_scan", "Letzter Scan", "Last scan"),
    ("col.last_seen", "Letztes gesehen", "Last seen"),
    ("col.interface", "Netzwerkinterface", "Network interface"),
    // Status
    ("status.alive", "Lebendig", "Alive"),
    ("status.no_response", "Keine Reaktion", "No response"),
    ("status.unknown", "Unbekannt", "Unknown"),
    ("status.scanning", "Wird gescannt", "Scanning"),
    ("sb.summary", "{a} lebendig, {d} keine Reaktion, {u} unbekannt", "{a} alive, {d} no response, {u} unknown"),
    ("sb.progress", "{c} / {t} geprüft | {a} lebendig | {p} %", "{c} / {t} checked | {a} alive | {p} %"),
    ("sb.paused", "Pausiert", "Paused"),
    ("sb.ready", "Bereit", "Ready"),
    ("sb.favorites", "{n} Favoriten", "{n} favorites"),
    ("vendor.randomized", "Private/Randomized", "Private/Randomized"),
    ("vendor.randomized_long", "Private / randomisierte MAC-Adresse", "Private / randomized MAC address"),
    // Device classes
    ("class.desktop", "Desktop-PC", "Desktop"),
    ("class.laptop", "Laptop", "Laptop"),
    ("class.server", "Server", "Server"),
    ("class.vm", "Virtuelle Maschine", "Virtual machine"),
    ("class.router", "Router", "Router"),
    ("class.firewall", "Firewall", "Firewall"),
    ("class.switch", "Switch", "Switch"),
    ("class.access_point", "Access Point", "Access point"),
    ("class.printer", "Drucker", "Printer"),
    ("class.nas", "NAS", "NAS"),
    ("class.camera", "Kamera", "Camera"),
    ("class.tv", "Fernseher", "Television"),
    ("class.media_player", "Mediaplayer", "Media player"),
    ("class.smartphone", "Smartphone", "Smartphone"),
    ("class.tablet", "Tablet", "Tablet"),
    ("class.iot", "IoT-Gerät", "IoT device"),
    ("class.home_automation", "Hausautomation", "Home automation"),
    ("class.game_console", "Spielkonsole", "Game console"),
    ("class.ups", "USV", "UPS"),
    ("class.unknown", "Unbekannt", "Unknown"),
    ("class.apple", "Apple-Gerät", "Apple device"),
    // Context menu
    ("ctx.open", "Öffnen", "Open"),
    ("ctx.shares", "SMB/Freigaben", "SMB/Shares"),
    ("ctx.tools", "Werkzeuge", "Tools"),
    ("ctx.ping", "Ping", "Ping"),
    ("ctx.traceroute", "Traceroute", "Traceroute"),
    ("ctx.portinfo", "Portinformationen", "Port information"),
    ("ctx.device", "Gerät", "Device"),
    ("ctx.add_fav", "Zu Favoriten hinzufügen", "Add to favorites"),
    ("ctx.remove_fav", "Aus Favoriten entfernen", "Remove from favorites"),
    ("ctx.rename", "Benutzername vergeben", "Assign custom name"),
    ("ctx.comment", "Kommentar bearbeiten", "Edit comment"),
    ("ctx.copy_mac", "MAC kopieren", "Copy MAC"),
    ("ctx.copy_ip", "IP kopieren", "Copy IP"),
    ("ctx.copy_name", "Name kopieren", "Copy name"),
    ("ctx.power", "Stromversorgung", "Power"),
    ("ctx.wol", "Wake-on-LAN", "Wake-on-LAN"),
    ("ctx.shutdown", "Herunterfahren...", "Shut down..."),
    ("ctx.reboot", "Neustarten...", "Restart..."),
    ("ctx.rescan", "Erneut scannen", "Rescan"),
    ("ctx.properties", "Eigenschaften", "Properties"),
    ("ctx.windows_only", "Nur unter Windows verfügbar", "Only available on Windows"),
    ("ctx.no_mac", "MAC-Adresse unbekannt", "MAC address unknown"),
    ("ctx.copy", "Kopieren", "Copy"),
    // Dialogs
    ("dlg.ok", "OK", "OK"),
    ("dlg.cancel", "Abbrechen", "Cancel"),
    ("dlg.close", "Schließen", "Close"),
    ("dlg.apply", "Übernehmen", "Apply"),
    ("dlg.start", "Start", "Start"),
    ("dlg.stop", "Stopp", "Stop"),
    ("dlg.copy_clip", "In Zwischenablage kopieren", "Copy to clipboard"),
    ("dlg.continuous", "Fortlaufend", "Continuous"),
    ("ping.title", "Ping", "Ping"),
    ("ping.reply", "Antwort von {ip}: {ms} ms", "Reply from {ip}: {ms} ms"),
    ("ping.timeout", "Zeitüberschreitung der Anforderung.", "Request timed out."),
    ("ping.unreach", "{ip}: Ziel nicht erreichbar.", "{ip}: Destination unreachable."),
    ("ping.stats", "Pakete: Gesendet {s}, Empfangen {r}, Verlust {l} %", "Packets: Sent {s}, Received {r}, Lost {l} %"),
    ("ping.rtt", "Min {min} ms | Avg {avg} ms | Max {max} ms", "Min {min} ms | Avg {avg} ms | Max {max} ms"),
    ("ping.no_icmp", "ICMP nicht verfügbar: {e}", "ICMP not available: {e}"),
    ("trace.title", "Traceroute", "Traceroute"),
    ("trace.done", "Ablaufverfolgung beendet.", "Trace complete."),
    ("trace.hint_perm", "Für Traceroute werden ICMP-Sockets benötigt. Unter Linux: net.ipv4.ping_group_range freigeben oder CAP_NET_RAW setzen.", "Traceroute needs ICMP sockets. On Linux allow net.ipv4.ping_group_range or grant CAP_NET_RAW."),
    ("ports.title", "Portinformationen", "Port information"),
    ("ports.none", "Keine offenen Ports gefunden.", "No open ports found."),
    ("edit.name_title", "Benutzername vergeben", "Assign custom name"),
    ("edit.comment_title", "Kommentar bearbeiten", "Edit comment"),
    ("edit.name_label", "Name für dieses Gerät:", "Name for this device:"),
    ("edit.comment_label", "Kommentar:", "Comment:"),
    ("wol.title", "Wake-on-LAN", "Wake-on-LAN"),
    ("wol.sent", "Magic Packet an {mac} gesendet ({n} Pakete).", "Magic packet sent to {mac} ({n} packets)."),
    ("wol.port", "UDP-Port:", "UDP port:"),
    ("power.confirm_shutdown", "Soll {host} wirklich heruntergefahren werden?", "Really shut down {host}?"),
    ("power.confirm_reboot", "Soll {host} wirklich neu gestartet werden?", "Really restart {host}?"),
    ("power.message", "Nachricht:", "Message:"),
    ("power.timeout", "Verzögerung (Sekunden):", "Delay (seconds):"),
    ("power.force", "Anwendungen ohne Nachfrage schließen", "Force applications to close"),
    ("power.done", "Befehl an {host} gesendet.", "Command sent to {host}."),
    ("power.failed", "Fehlgeschlagen: {e}", "Failed: {e}"),
    // Options
    ("opt.title", "Optionen", "Options"),
    ("opt.tab_perf", "Leistung", "Performance"),
    ("opt.tab_scan", "Scan", "Scan"),
    ("opt.tab_names", "Namensauflösung", "Name resolution"),
    ("opt.tab_programs", "Programme", "Programs"),
    ("opt.tab_ui", "Oberfläche", "Interface"),
    ("opt.tab_net", "Netzwerk", "Network"),
    ("opt.profile", "Scanprofil:", "Scan profile:"),
    ("opt.profile_fast", "Schnell", "Fast"),
    ("opt.profile_standard", "Standard", "Standard"),
    ("opt.profile_accurate", "Genau", "Accurate"),
    ("opt.profile_custom", "Benutzerdefiniert", "Custom"),
    ("opt.ports", "Ports:", "Ports:"),
    ("opt.icmp_timeout", "ICMP-Timeout:", "ICMP timeout:"),
    ("opt.tcp_timeout", "TCP-Timeout (ms):", "TCP timeout (ms):"),
    ("opt.name_timeout", "Namensauflösung Timeout (ms):", "Name resolution timeout (ms):"),
    ("opt.http_timeout", "HTTP-Timeout (ms):", "HTTP timeout (ms):"),
    ("opt.hosts", "Gleichzeitige Hosts:", "Concurrent hosts:"),
    ("opt.portconc", "Gleichzeitige Port-Prüfungen:", "Concurrent port checks:"),
    ("opt.dns", "Reverse DNS", "Reverse DNS"),
    ("opt.netbios", "NetBIOS", "NetBIOS"),
    ("opt.mdns", "mDNS / DNS-SD", "mDNS / DNS-SD"),
    ("opt.llmnr", "LLMNR", "LLMNR"),
    ("opt.http_meta", "HTTP-/TLS-Metadaten lesen", "Read HTTP/TLS metadata"),
    ("opt.banners", "Dienst-Banner lesen", "Read service banners"),
    ("opt.ipp", "IPP-Druckerinformationen", "IPP printer information"),
    ("opt.dead", "Namen nicht antwortender Hosts auflösen", "Resolve names of hosts that do not respond"),
    ("opt.snmp", "SNMP-Community (leer = aus):", "SNMP community (empty = off):"),
    ("opt.snmp_hint", "Nur lesend, keine automatischen Community-Versuche.", "Read-only, no automatic community guessing."),
    ("opt.clear", "Ergebnisse vor jedem Scan leeren", "Clear results before each scan"),
    ("opt.csv_bom", "CSV mit BOM (Excel)", "CSV with BOM (Excel)"),
    ("opt.programs_hint", "Platzhalter: {ip}, {host}, {port}. Leer = Standardprogramm des Systems.", "Placeholders: {ip}, {host}, {port}. Empty = system default."),
    ("opt.interface", "Netzwerkinterface für IP-Schaltfläche:", "Network interface for IP button:"),
    ("opt.auto", "Automatisch (Standardroute)", "Automatic (default route)"),
    ("opt.hide_virtual", "Virtuelle Adapter ausblenden", "Hide virtual adapters"),
    ("opt.theme", "Design:", "Theme:"),
    ("opt.theme_light", "Hell", "Light"),
    ("opt.theme_dark", "Dunkel", "Dark"),
    ("opt.theme_system", "System", "System"),
    ("opt.wol_port", "Wake-on-LAN Port:", "Wake-on-LAN port:"),
    ("opt.invalid_ports", "Ungültige Portliste", "Invalid port list"),
    // MAC DB / fingerprints / diagnostics / about
    ("macdb.title", "MAC-Datenbank", "MAC database"),
    ("macdb.version", "Version:", "Version:"),
    ("macdb.source", "Quelle:", "Source:"),
    ("macdb.entries", "Einträge (MA-L / MA-M / MA-S):", "Entries (MA-L / MA-M / MA-S):"),
    ("macdb.import", "IEEE-CSV importieren...", "Import IEEE CSV..."),
    ("macdb.reset", "Mitgelieferte Datenbank verwenden", "Use bundled database"),
    ("macdb.lookup", "MAC nachschlagen:", "Look up MAC:"),
    ("macdb.imported", "{n} Einträge importiert.", "{n} entries imported."),
    ("macdb.hint", "Download: standards-oui.ieee.org (oui.csv, mam.csv, oui36.csv). Der Import erfolgt lokal.", "Download: standards-oui.ieee.org (oui.csv, mam.csv, oui36.csv). Import is local."),
    ("fp.title", "Fingerprints", "Fingerprints"),
    ("fp.count", "{n} Regeln aktiv", "{n} rules active"),
    ("fp.user_file", "Eigene Regeln:", "Custom rules:"),
    ("fp.reload", "Neu laden", "Reload"),
    ("fp.open_folder", "Ordner öffnen", "Open folder"),
    ("diag.title", "Diagnose", "Diagnostics"),
    ("about.title", "Über IP Scanner Deluxe", "About IP Scanner Deluxe"),
    ("about.text", "Schneller, lokaler Netzwerkscanner für Windows, Linux und macOS.\nAlle Daten bleiben auf diesem Computer: keine Telemetrie, keine Cloud-Abfragen.", "Fast, local network scanner for Windows, Linux and macOS.\nAll data stays on this computer: no telemetry, no cloud lookups."),
    ("about.oui", "Herstellerdaten: IEEE Registration Authority (öffentliche Registrierungsdaten).", "Vendor data: IEEE Registration Authority (public registry data)."),
    // Details / properties
    ("det.title", "Eigenschaften", "Properties"),
    ("det.names", "Namen", "Names"),
    ("det.detection", "Erkennung", "Detection"),
    ("det.services", "Dienste", "Services"),
    ("det.first_seen", "Zuerst gesehen", "First seen"),
    ("unknown", "unbekannt", "unknown"),
    ("not_implemented", "Noch nicht implementiert", "Not implemented yet"),
    ("err.range", "Ungültiger Bereich", "Invalid range"),
    ("err.export", "Export fehlgeschlagen: {e}", "Export failed: {e}"),
    ("err.load", "Laden fehlgeschlagen: {e}", "Loading failed: {e}"),
    ("msg.exported", "Gespeichert: {p}", "Saved: {p}"),
    ("msg.copied", "In Zwischenablage kopiert", "Copied to clipboard"),
    ("hist.new", "Neu im Netzwerk", "New on network"),
    ("hist.ip_changed", "IP geändert (vorher {old})", "IP changed (was {old})"),
    ("hist.name_changed", "Hostname geändert (vorher {old})", "Hostname changed (was {old})"),
    ("fav.scan", "Favoriten scannen", "Scan favorites"),
    ("fav.remove", "Favorit entfernen", "Remove favorite"),
];

pub fn t(key: &str) -> &'static str {
    let en = is_english();
    for (k, de, eng) in STRINGS {
        if *k == key {
            return if en { eng } else { de };
        }
    }
    tracing_missing(key);
    // Leak-free fallback: unknown keys map to a static marker.
    "??"
}

fn tracing_missing(_key: &str) {
    #[cfg(debug_assertions)]
    eprintln!("missing translation key: {_key}");
}

/// Translate with `{name}` placeholders.
pub fn tf(key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(key).to_string();
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_unique() {
        let mut seen = std::collections::HashSet::new();
        for (k, _, _) in STRINGS {
            assert!(seen.insert(*k), "duplicate key {k}");
        }
    }

    #[test]
    fn placeholders() {
        set_language(Language::German);
        assert_eq!(tf("sb.summary", &[("a", "5"), ("d", "0"), ("u", "0")]), "5 lebendig, 0 keine Reaktion, 0 unbekannt");
    }
}
