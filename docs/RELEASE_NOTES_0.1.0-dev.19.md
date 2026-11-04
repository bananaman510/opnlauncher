# OpnLauncher 0.1.0-dev.19

This is an unreleased development build. It adds an optional first-step Fabric code bridge for the native Rust Horizon client.

## Experimental Fabric event mapping

- A per-profile switch can start Fabric Loader in a helper JVM while the Rust Horizon game is running.
- Fabric main/client entrypoints are loaded and Horizon client ticks are forwarded to Fabric API's `END_CLIENT_TICK` event using a bounded loopback connection.
- The feature is disabled by default and reads existing mod JARs without changing them.
- It is not a Java-to-Rust compiler. Callbacks get an empty Minecraft client shell; world/player state, renderer APIs, Mixin behavior, and Sodium are not supported. Real-mod and in-game validation remains to be done.
- Java mod code runs with the user's system permissions and is not sandboxed. Enable it only for trusted mods.

The separate Java + Fabric mode remains available and starts the actual Java Minecraft client. The resource-only adapter remains separate from both options.
