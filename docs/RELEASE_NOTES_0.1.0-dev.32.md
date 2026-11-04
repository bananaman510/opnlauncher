# OpnLauncher 0.1.0-dev.32 development preview

This is an unreleased development build. It is not a stable release. Horizon's experimental Fabric bridge remains a narrow API mapping and does not provide general Fabric mod compatibility.

## Horizon Fabric bridge

- Mapped `Entity.sendSystemMessage(Component)` to Horizon's Rust system chat.
- System messages are delivered as plain text and follow Horizon's chat visibility setting. Styling, click events, and other rich component behavior are not translated.
- Extended the unchanged development probe so its Fabric keybinding callback sends a system-chat message alongside an action-bar message and bounded player actions.
- Extended the Rust protocol parser, static Minecraft API allowlist, facade contract, and loopback test for the new action.
- The bridge remains optional and off by default. Unsupported APIs, Mixins, access wideners, extra dependencies, and Sodium remain unsupported.

## Validation

- `npm run build` passed.
- `npm test` passed: 10 frontend tests.
- `cargo test --locked --no-default-features --lib --manifest-path src-tauri/Cargo.toml --offline` passed: 70 Rust tests.
- `npm run test:fabric-bridge` passed: the Java facade contract, five Rust protocol tests, and Rust/Java loopback.
- `npm run horizon:build:dev` and `npm run tauri -- build --no-bundle` passed. The desktop build compiled debug and optimized Horizon runtimes and produced the `.32` Tauri executable.
- These checks use a probe and synthetic player state. No AppImage, live Horizon session, or third-party Fabric mod was tested.
