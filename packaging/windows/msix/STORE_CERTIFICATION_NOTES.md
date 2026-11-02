# Notes for Microsoft Store certification

Text for the "Notes for certification" field in Partner Center
(Submission > Submission options). Paste the block below.

---

IP Scanner Deluxe is a native Win32 desktop application for network
administration and diagnostics, written in Rust. It discovers devices on the
local network and shows their IP address, host name, MAC address, vendor and
the network services they offer.

Technical background:

- The application uses standard Windows networking APIs: the IP Helper API
  (IcmpCreateFile/IcmpSendEcho for ping, SendARP and GetIpNetTable for
  MAC address resolution) and Winsock (TCP and UDP sockets) for host
  discovery, connectivity tests, name resolution (DNS, NetBIOS, LLMNR, mDNS)
  and service detection (HTTP/HTTPS, SMB, RDP, SSH and others).
- Scanning is limited to the address ranges the user enters and is meant for
  networks the user administers. It does not run in the background.
- Normal operation does not require administrator privileges. The package
  does not request elevation.
- The application installs no kernel drivers, packet capture drivers (such as
  Npcap or WinPcap), NDIS components or Windows services, and creates no
  autostart entries.
- The runFullTrust restricted capability is required because the
  application is an existing classic Win32 desktop application
  (packagedClassicApp, mediumIL) that uses the Win32 networking APIs listed
  above. It is not an AppContainer app.
- Settings, favorites and logs are stored in the user's application data
  folder. Nothing is written to the installation folder. The application has
  no telemetry and does not send data to any server.
- Updates are delivered only through the Microsoft Store. The Store build has
  no self-update mechanism.

Remote shutdown and restart:

- Shutting down or restarting a remote Windows computer is always started
  explicitly by the user from the context menu of a host and requires a
  confirmation dialog.
- The command runs with the permissions of the currently signed-in Windows
  user. The application does not ask for, store or transmit Windows
  credentials and does not elevate privileges.

How to test:

1. Start IP Scanner Deluxe. The address range of the local network is
   filled in automatically.
2. Click "Scan". Devices on the network are listed with name, IP address,
   manufacturer and MAC address.
3. Right-click a device to open services (for example HTTP or RDP), send a
   Wake-on-LAN packet or show its properties.
4. Help > About shows the version, the license (MIT OR Apache-2.0) and the
   third-party licenses.

A network with at least one other device gives the most meaningful results.
No account, sign-in or test credentials are needed.

The application is open source (MIT OR Apache-2.0):
https://github.com/ray77/ip-scanner-deluxe
