# OpnLauncher 0.1.0-dev.20

This is an unreleased development build. It strengthens the experimental Fabric lifecycle bridge for native Horizon and adds an automated Java contract check.

## Fabric event mapping prototype

- Provides a small Fabric API 26.3 facade for the event-registration methods used by `ClientTickEvents.END_CLIENT_TICK`.
- Runs allowed Java entrypoints in a helper JVM and forwards Horizon's client end-tick event through a bounded loopback queue.
- Scans mod metadata and Java constant pools, checks entrypoint class paths, rejects duplicate mod ids, and stages only unchanged JAR copies that pass the current allowlist.
- Keeps the setting off by default. Original mod JARs are not rewritten, replaced, or removed.
- Adds Rust scanner/staging tests and a Java contract test using a test loader stub.

## Limits

This is not a general Fabric compatibility layer and does not translate arbitrary Java bytecode or Minecraft APIs into Rust. Callbacks receive `null` for the Minecraft client parameter, so ordinary gameplay mods that access player/world state, input, rendering, or game methods are unsupported. Mixins, access wideners, Sodium, unrecognized Fabric APIs, custom entrypoint adapters, and extra dependencies are rejected. The test loader stub does not prove compatibility with a real Fabric Loader installation, and this feature has not been validated in live Horizon gameplay.

The optional Java + Fabric runtime remains the supported way to run compatible Java Fabric mods on the Java Minecraft client. Native Horizon mods use Horizon's Rust `.hmod` system.
