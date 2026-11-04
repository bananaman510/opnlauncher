# OpnLauncher 0.1.0-dev.27

This is an unreleased development build. The optional Fabric code bridge remains experimental and disabled by default.

## Default keyboard key mappings

Unchanged Fabric mod JARs that stay within the currently mapped API can register a default keyboard key through `KeyMappingHelper.registerKeyMapping(...)`. Horizon translates physical key names into Minecraft 26.3's SDL/HID key values and forwards bounded held-key snapshots and non-repeating key-press edges from its Rust game loop. Mod callbacks can query `KeyMapping.isDown()` or consume a press with `KeyMapping.consumeClick()`. The probe uses **P** to demonstrate both behaviors.

The key mapping facade supports keyboard inputs only and uses each mapping's declared default. It does not yet integrate Minecraft's Options screen or provide rebinding, key-conflict handling, mouse/controller mappings, Mixin support, or general Fabric API behavior. Mods still execute as Java code in a separate helper process; this maps selected API calls and does not compile or translate arbitrary Java bytecode into Rust. The bridge remains opt-in. A live Horizon session and third-party mod have not been verified.

## Checks

- `npm run build` and all 10 frontend tests passed.
- The Java bridge contract passed with a probe JAR that checks tick order, read-only snapshots, default key registration, `isDown()`, `consumeClick()`, and action-bar output.
- All 70 launcher Rust library tests passed after correcting the key-category allowlist. The frontend build and 10 frontend tests passed.
- The Rust/Java loopback transport test passed when rerun with permission to bind its temporary localhost socket. The probe received synthetic held-key and press-edge values through the production bridge and returned its action-bar response.
- A separate integration run with actual Fabric Loader 0.20.1 loaded the probe, resolved the built-in key-mapping module, and delivered synthetic held-key and press-edge values through the bridge.
- The development and optimized Horizon runtimes and Java helper JAR built successfully. The desktop executable also compiled. AppImage bundling failed at `linuxdeploy`, so no `.27` AppImage was produced. No live Horizon session or third-party mod has been tested for this change.
