# Privacy

OpnLauncher is designed to keep launcher data on the user's device. This 0.1.0 development prototype has no analytics, telemetry, advertising, tracking pixels, crash-reporting service, launcher account, or automatic diagnostic upload.

## Network activity in this version

The current prototype's launcher UI and backend do not make application network requests. They check local Minecraft directories, inspect local Java executables, and start a local game process. The UI uses system fonts and makes no remote font request. A launched Minecraft game is a separate process and may make its own requests according to that game's behavior; OpnLauncher does not intercept or firewall game traffic.

Future features may contact third parties when explicitly used. Microsoft sign-in will contact Microsoft's authentication services and Xbox/Minecraft services. Minecraft metadata and assets will use official Mojang/Microsoft endpoints. Mod search may contact Modrinth's documented API. The app must describe these requests before those features ship. OpnLauncher-operated servers are not part of the design.

## Local records

The app data directory contains the SQLite database, launcher logs, generated native libraries, and any empty instance directories created by the user. Registered external game directories are stored as paths and are not copied. Diagnostic exports, when added, must be manual and redact authentication secrets.

Development-only local profiles are unverified. Their usernames and stable UUIDs remain in the local database and are not sent to an OpnLauncher service.
