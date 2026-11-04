# OpnLauncher 0.1.0-dev.28

This is an unreleased development build. The optional Fabric code bridge is experimental and remains disabled by default.

## One mapped player action

The unchanged Fabric probe now calls `Entity.setSprinting(boolean)` when its default **P** key is pressed. OpnLauncher carries that request from the helper JVM through bounded local IPC and applies it on Horizon's Rust game thread by updating Horizon's native sprint-key state. The Rust side ignores sprint requests unless a local player is actively in the game. The probe also shows the resulting state in Horizon's HUD.

This is an explicit API mapping, not a general Java-to-Rust translator. The supported bridge subset includes the previously mapped client tick callbacks, player and world-clock snapshots, default keyboard bindings, HUD action-bar messages, and this sprint request. Fabric mods only work when their entrypoint code and every referenced Minecraft/Fabric API member fit the static allowlist and mapped facades. Mixins, Sodium, rendering, arbitrary Java APIs, most player/world mutations, rebinding, mouse/controller mappings, and general gameplay-mod compatibility are unsupported. Java code runs with the user's operating-system permissions.

## Checks

- `npm run build` passed, and all 10 frontend tests passed.
- All 70 OpnLauncher Rust library tests passed.
- The Java bridge contract and Rust-to-Java loopback passed using the unchanged development probe. The harness verified ordered tick callbacks, player/world-clock snapshots, key registration and polling, press-edge handling, sprint requests, HUD responses, and rejection of unsupported events.
- Development and optimized Horizon runtimes built successfully. `npm run tauri -- build --no-bundle` built the Linux desktop executable.
- The upstream `horizon-client` unit-test command could not compile its test target because vendored Horizon test code includes a `native-hooks.json` file that is absent from the repository. The edited Horizon runtime compiled successfully in both runtime build profiles.
- No AppImage was produced for this version. The desktop executable was not opened for a live game session, and no third-party Fabric mod has been verified in Horizon.
