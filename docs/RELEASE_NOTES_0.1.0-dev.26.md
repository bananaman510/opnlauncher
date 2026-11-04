# OpnLauncher 0.1.0-dev.26

This is an unreleased development build. The optional Fabric-to-Horizon bridge remains experimental and narrowly mapped.

## Read-only world-clock mapping

- Added `Minecraft.level.getOverworldClockTime()` backed by a snapshot captured from Horizon's Rust game state.
- Updated the bridge probe and Rust/Java loopback checks to verify player and world-clock data reach unchanged Fabric code.
- The class scanner accepts this exact Minecraft field and method and rejects unmapped methods such as `getDayTime()`.
- Updated Horizon documentation to record the official Fabric Loader test and the remaining live-game and third-party-mod gaps.

Fabric Mixins, Sodium, rendering, world mutation, other lifecycle events, and general Fabric API compatibility remain unsupported. Live Horizon gameplay has not been verified.
