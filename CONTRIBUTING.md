# Contributing

OpnLauncher is an early development project. The public GitHub snapshot is version `0.1.0-dev.16`; it is not stable and behavior or local data formats may change. Contributions should fit the local-first, privacy-preserving scope and must not add telemetry, advertising, hidden network requests, authentication bypasses, or bundled Minecraft assets.

## Development

1. Install Rust 1.90 or later, Node.js 22.12 or later, npm, and the Linux dependencies in the README.
2. Run `npm ci` to install the versions pinned by `package-lock.json`.
3. Start the desktop app with `npm run tauri -- dev`.
4. Run the frontend checks with `npm run build` and `npm run test`.
5. Run Rust formatting and core tests with the commands in the README. The GitHub CI workflow runs the Linux desktop check as well.

Keep changes focused, document new network behavior and configuration, and include tests for important behavior. Do not include personal account data, local databases, authentication tokens, or proprietary game files in contributions.

## Git workflow

Forgejo remains the primary development source of truth and `main` is its integration branch. The public GitHub copy is a deliberate snapshot; it is not automatically mirrored. When the GitHub repository is available, use focused pull requests and issues there for public collaboration. Maintainers review contributions and decide which changes to integrate into Forgejo. Do not include private repository URLs, personal data, local databases, game files, credentials, or authentication tokens in public contributions.

Use short-lived branches for substantial work and keep pull requests focused. Include the user-visible behavior, relevant test results, and known limitations in the pull request description. Do not create release tags or publish releases; maintainers handle those separately after review.
