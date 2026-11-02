# Microsoft Store listing

Texts and images for Partner Center > Store listings, ready to paste.
Screenshots are 1366×768 PNG (Store minimum for desktop), generated from the
demo data:

```bash
cargo run -p ip-scanner-deluxe -- --english --demo --size 1366x768 --screenshot packaging/windows/store-listing/en-us/1-results.png
cargo run -p ip-scanner-deluxe -- --english --demo --expand --size 1366x768 --screenshot packaging/windows/store-listing/en-us/2-services.png
cargo run -p ip-scanner-deluxe -- --english --demo --expand --theme dark --size 1366x768 --screenshot packaging/windows/store-listing/en-us/3-dark.png
```

(Same without `--english` into `de-de/`.) Store logos, used for both
languages: `PosterArt-720x1080.png` / `PosterArt-1440x2160.png` (9:16 poster
art), `BoxArt-1080x1080.png` / `BoxArt-2160x2160.png` (1:1 box art) and
`StoreLogo-300x300.png` (1:1 app tile icon).

Rules: describe the app as a network administration and diagnostics tool. No
other products' or companies' names in name, description or search terms; no
"hacking" wording.

## Common

| Field                | Value |
|----------------------|-------|
| Name                 | IP Scanner Deluxe |
| Category             | Utilities & tools |
| Privacy policy URL   | https://github.com/ray77/ip-scanner-deluxe/blob/main/PRIVACY.md |
| Website              | https://github.com/ray77/ip-scanner-deluxe |
| Support contact      | https://github.com/ray77/ip-scanner-deluxe/issues |
| Copyright            | Copyright © 2026 Dominik Schmidt |
| Pricing              | Free |

## English (en-us)

**Short description**

Fast, free network scanner for your local network: find devices, MAC addresses and open services.

**Description**

```text
IP Scanner Deluxe finds the devices on your local network within seconds and shows their name, IP address, manufacturer and MAC address.

Features:
• Network discovery for IPv4 and IPv6 address ranges
• MAC address and manufacturer lookup with a built-in offline database
• Detects web interfaces (HTTP/HTTPS), shared folders, Remote Desktop, SSH, FTP and printers
• Port scan with configurable port lists
• Wake on LAN
• Open Remote Desktop, SSH, the browser or shared folders directly from the list
• Shut down or restart Windows computers on your network (always with confirmation)
• Favorites, comments and export to CSV, HTML, JSON and XML
• Light and dark theme, English and German

Privacy: no account, no telemetry, no cloud. All data stays on your computer.

IP Scanner Deluxe is free and open source (MIT OR Apache-2.0).
Only scan networks you are authorized to administer.
```

**What's new in this version** – copy from the GitHub release notes.

**Features** (up to 20, one per line)

```text
Network discovery for IPv4 and IPv6
MAC address and manufacturer lookup
Detection of HTTP/HTTPS, shared folders, RDP, SSH, FTP and printers
Port scan
Wake on LAN
Remote shutdown and restart with confirmation
Favorites and comments
Export to CSV, HTML, JSON and XML
No telemetry, all data stays local
Free and open source
```

**Search terms** (max. 7)

```text
IP scanner
LAN scanner
network scanner
network discovery
port scanner
MAC address
Wake on LAN
```

**Screenshot captions**

1. `en-us/1-results.png` – All devices on your network at a glance
2. `en-us/2-services.png` – Services of each device, ready to open
3. `en-us/3-dark.png` – Dark theme

## Deutsch (de-de)

**Kurzbeschreibung**

Schneller, kostenloser Netzwerkscanner für Ihr lokales Netzwerk: Geräte, MAC-Adressen und Dienste finden.

**Beschreibung**

```text
IP Scanner Deluxe findet die Geräte in Ihrem lokalen Netzwerk in Sekunden und zeigt Name, IP-Adresse, Hersteller und MAC-Adresse.

Funktionen:
• Netzwerksuche in IPv4- und IPv6-Adressbereichen
• MAC-Adresse und Hersteller über eine eingebaute Offline-Datenbank
• Erkennt Weboberflächen (HTTP/HTTPS), Freigaben, Remotedesktop, SSH, FTP und Drucker
• Portscan mit einstellbaren Portlisten
• Wake-on-LAN
• Remotedesktop, SSH, Browser oder Freigaben direkt aus der Liste öffnen
• Windows-Rechner im Netzwerk herunterfahren oder neu starten (immer mit Rückfrage)
• Favoriten, Kommentare und Export nach CSV, HTML, JSON und XML
• Helles und dunkles Design, Deutsch und Englisch

Datenschutz: kein Konto, keine Telemetrie, keine Cloud. Alle Daten bleiben auf Ihrem Computer.

IP Scanner Deluxe ist kostenlos und Open Source (MIT OR Apache-2.0).
Scannen Sie nur Netzwerke, die Sie verwalten dürfen.
```

**Neuerungen in dieser Version** – aus den GitHub-Release-Notes übernehmen
(übersetzt).

**Produktfeatures** (bis zu 20, eine pro Zeile)

```text
Netzwerksuche für IPv4 und IPv6
MAC-Adresse und Hersteller
Erkennung von HTTP/HTTPS, Freigaben, RDP, SSH, FTP und Druckern
Portscan
Wake-on-LAN
Herunterfahren und Neustart entfernter Rechner mit Rückfrage
Favoriten und Kommentare
Export nach CSV, HTML, JSON und XML
Keine Telemetrie, alle Daten bleiben lokal
Kostenlos und Open Source
```

**Suchbegriffe** (max. 7)

```text
IP Scanner
LAN Scanner
Netzwerkscanner
Netzwerk scannen
Portscanner
MAC-Adresse
Wake on LAN
```

**Bildunterschriften**

1. `de-de/1-results.png` – Alle Geräte im Netzwerk auf einen Blick
2. `de-de/2-services.png` – Die Dienste jedes Geräts, direkt zu öffnen
3. `de-de/3-dark.png` – Dunkles Design
