# Public development preview checklist

Target source version: `0.1.0-dev.17` (pre-release; not stable).

This checklist prepares a public source snapshot. It does not authorize creating a GitHub repository, adding a GitHub remote, uploading a public snapshot, creating a tag, or publishing a release.

## Prepared in this branch

- [x] Preserve the private Forgejo remote as the primary development source; no automatic GitHub mirror is configured.
- [x] Keep version metadata at `0.1.0-dev.17` and label the preview as unstable.
- [x] Document Linux setup, local developer accounts, Java instances, mods, Horizon, privacy, storage, and known limitations.
- [x] Add draft preview notes, contributor guidance, and third-party notices.
- [x] Add CI, dependency security checks, and a manually started AppImage artifact workflow. None creates a tag or GitHub release.
- [x] Keep the existing GPL-3.0-or-later license and Mojang/Microsoft independence disclaimer.
- [x] Inventory declared Rust dependency licenses for Linux, Windows, and macOS in both Cargo lockfiles; no resolved package is missing a license declaration. Review the GPLv3 compatibility notes and preserve the original MPL/CDLA terms.
- [x] Run a repository-history credential-pattern review and source-level privacy/security review for this preparation.
- [x] Build the Linux AppImage and inspect its bundled license and attribution resources. It has not been launched from this review environment.

## Complete before public upload

- [ ] Run the new workflows on the intended GitHub repository and resolve any failures.
- [ ] Verify that the AppImage starts on the target Linux environment.
- [ ] Re-run the full secret scan and review the exact commit and complete Git history immediately before upload.
- [ ] Create the GitHub repository and add a separate `github` remote only after explicit approval. Keep Forgejo as `origin` and the primary development remote.
- [ ] Enable and verify GitHub private vulnerability reporting before inviting security reports.
- [ ] Obtain explicit approval for pushing the exact reviewed commit, creating `v0.1.0-dev.17`, and publishing the GitHub pre-release. Do not rewrite history.

The maintainers have reported successful local AppImage and Horizon launches. The current preparation work does not independently reproduce a complete Minecraft session or establish cross-platform compatibility.
