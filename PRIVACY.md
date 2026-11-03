# Privacy

OpnLauncher is designed to keep launcher data on the user's device. This 0.1.0 development build has no analytics, telemetry, advertising, tracking pixels, crash-reporting service, launcher account, or automatic diagnostic upload.

## Network activity in this version

Opening the **Minecraft versions** section requests Mojang's official version manifest from `piston-meta.mojang.com`. Installing a version requests the selected version metadata and the client, library, logging, and asset files referenced by that metadata, including `libraries.minecraft.net` and `resources.download.minecraft.net`. These requests occur only when the user opens or refreshes the version list or starts an installation. Version downloads use HTTPS and verify upstream SHA-1 values and declared sizes where provided.

Opening **Mods** for an instance with readable Minecraft metadata searches Modrinth's documented API when Modrinth is selected. Selecting CurseForge makes requests to its official API only after the user enters a personal API key. That key is held in frontend memory for the current app session and sent only to CurseForge in the required request header; it is not saved to SQLite, WebView preferences, or logs, and is not sent to OpnLauncher. Mod files are fetched from the providers' supported HTTPS file hosts and checked against their published SHA-1 and/or file size before installation.

Opening an instance's **Game setup** requests Mojang's official version manifest. Choosing Fabric or Forge also requests compatible loader versions from Fabric Meta (`meta.fabricmc.net`) or Forge Maven (`maven.minecraftforge.net`). Installing Fabric downloads its selected profile and required library artifacts from `maven.fabricmc.net` and, when referenced by the profile, `libraries.minecraft.net`. The launcher uses profile checksums and sizes when available; for coordinate-based profile entries it downloads the artifact's Maven SHA-1 sidecar from that same trusted host and enforces a file-size limit. Installing Forge downloads the official installer and its `.sha1` from Forge Maven, then runs that installer for the selected game folder. The Forge installer may make its own requests to Forge and Mojang metadata and library hosts while completing installation. These requests happen only after the user opens setup or chooses Install; they do not go through an OpnLauncher-operated server.

Project artwork is loaded from `cdn.modrinth.com` and `*.forgecdn.net` when project cards or galleries are shown. These image requests reveal normal connection information such as the user's IP address to the relevant content host. The app permits images only from these hosts. Search text, the selected Minecraft version, and loader are sent to the chosen provider so it can return compatible results. Requests are not sent to an OpnLauncher-operated service.

The UI uses system fonts and makes no remote font request. A launched Minecraft game is a separate process and may make its own requests according to that game's behavior; OpnLauncher does not intercept or firewall game traffic. Microsoft authentication is not implemented yet.

Future features may contact third parties when explicitly used. Microsoft sign-in will contact Microsoft's authentication services and Xbox/Minecraft services. OpnLauncher-operated servers are not part of the design.

## Local records

The app data directory contains the SQLite database, launcher logs, generated native libraries, and managed instance directories. A downloaded vanilla version is stored inside its new managed instance directory. Registered external game directories are stored as paths and are not copied or modified by the general version browser. Game setup adds selected Minecraft and loader files to the chosen instance's game folder after the user chooses Install; conflicting existing files are left untouched. A registered folder may be shared by multiple instance records, and the UI warns when this is detected. The Forge installer may temporarily update `launcher_profiles.json`; OpnLauncher restores its prior contents (or removes the temporary file) after that installer exits. Loader installs do not remove worlds or mods. Diagnostic exports, when added, must be manual and redact authentication secrets.

Local developer profiles are unverified and are available in development and release builds. Their usernames and stable UUIDs remain in the local database and are not sent to an OpnLauncher service. They do not authenticate with Microsoft or verify game ownership.

The desktop WebView also stores local preferences on this device: theme, accent color, default Java path, default memory allocation, added Java executable paths, favorite instance IDs, and the currently selected instance and local profile. These preferences are not sent to a server. Removing an instance from the launcher also removes its favorite marker; it does not delete its game files or worlds.
