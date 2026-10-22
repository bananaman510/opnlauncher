# Contributing

OpnLauncher is in early development. Contributions should fit the local-first, privacy-preserving scope and should not add telemetry, advertising, hidden network requests, authentication bypasses, or bundled Minecraft assets.

## Development

1. Install Rust, Node.js/npm, and the Linux dependencies in the README.
2. Run `npm install`.
3. Start the desktop app with `npm run tauri -- dev`.
4. Run the frontend checks with `npm run build` and `npm run test`.
5. Run core Rust tests with `cargo test --no-default-features --lib --manifest-path src-tauri/Cargo.toml`.

Keep changes focused, document new network behavior and configuration, and include tests for important behavior. Do not include personal account data, local databases, authentication tokens, or proprietary game files in contributions.

## Git workflow

The private self-hosted Forgejo repository is the development source of truth. `main` is the primary branch. Do not configure GitHub mirroring or publish the repository. Do not create tags or releases without explicit maintainer approval.
