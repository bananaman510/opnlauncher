# Privacy

OpnLauncher is designed to keep launcher data on the user's device. This 0.1.0 development build has no analytics, telemetry, advertising, tracking pixels, crash-reporting service, launcher account, or automatic diagnostic upload.

## Network activity in this version

Opening the **Minecraft versions** section requests Mojang's official version manifest from `piston-meta.mojang.com`. Installing a version requests the selected version metadata and the client, library, logging, and asset files referenced by that metadata, including `libraries.minecraft.net` and `resources.download.minecraft.net`. These requests occur only when the user opens or refreshes the version list or starts an installation. Version downloads use HTTPS and verify upstream SHA-1 values and declared sizes where provided. Requests do not include account tokens or launcher profile names; the contacted hosts receive normal connection information such as the user's IP address.

The UI uses system fonts and makes no remote font request. A launched Minecraft game is a separate process and may make its own requests according to that game's behavior; OpnLauncher does not intercept or firewall game traffic. Microsoft authentication and Modrinth requests are not implemented yet.

Future features may contact third parties when explicitly used. Microsoft sign-in will contact Microsoft's authentication services and Xbox/Minecraft services. Mod search may contact Modrinth's documented API. OpnLauncher-operated servers are not part of the design.

## Local records

The app data directory contains the SQLite database, launcher logs, generated native libraries, and managed instance directories. A downloaded vanilla version is stored inside its new managed instance directory. Registered external game directories are stored as paths and are not copied or modified by the downloader. Diagnostic exports, when added, must be manual and redact authentication secrets.

Local developer profiles are unverified and are available in development and release builds. Their usernames and stable UUIDs remain in the local database and are not sent to an OpnLauncher service. They do not authenticate with Microsoft or verify game ownership.

The desktop WebView also stores small interface preferences on this device: the selected theme and accent color, favorite instance IDs, and the currently selected instance and local profile. These preferences are not sent to a server. Removing an instance from the launcher also removes its favorite marker; it does not delete its game files or worlds.
