# OpnLauncher 0.1.0-dev.22

This is an unreleased development build. The optional Fabric code bridge now provides a second small gameplay-facing API mapping for native Rust Horizon.

## Read-only local-player snapshot

- Fabric `END_CLIENT_TICK` callbacks can read `Minecraft.player` when Horizon has a local player.
- The mapped getters expose player name, X/Y/Z position, health, alive state, sprinting, and on-ground state.
- Rust captures the values on Horizon's game thread and sends a bounded snapshot to the Java helper; Java mods cannot write these values back or directly mutate Horizon.
- The development probe now displays the values in Horizon's native action bar so a user can check the end-to-end path in a real launch.
- Fabric JARs remain unchanged. The bridge still loads accepted JARs beside Horizon in a helper JVM; it does not convert arbitrary bytecode to Rust.

## Limits and validation

This remains a narrow experimental mapping. World access, other entities, input, registries, rendering, Mixins, Sodium, and player mutation are unsupported. The scanner only stages files that use explicitly mapped classes and methods. The test-loader and Rust IPC contracts pass, but the mapped player snapshot still needs verification in an actual Horizon session using a real Fabric Loader and the unchanged probe JAR. This does not establish compatibility for ordinary Fabric gameplay mods.
