# Third-party notices

OpnLauncher-authored launcher code is licensed under GPL-3.0-or-later; see [LICENSE](LICENSE). Third-party components retain their own licenses. This file summarizes the direct frontend dependencies and the vendored components that need prominent attribution. Transitive dependency versions are pinned in `package-lock.json` and the Cargo lockfiles; consult the package or crate license files for their complete terms.

## Vendored Horizon engine

The included Horizon source is from upstream commit `fdd290b99b4459cb4803487758ba8015a45aa47f` and is licensed under MIT. Its license is preserved at `src-tauri/horizon-upstream/LICENSE`. The small `opnlauncher-horizon-runtime` wrapper crate also declares MIT in its Cargo manifest.

Horizon's additional third-party attributions and license texts are preserved at `src-tauri/horizon-upstream/THIRD_PARTY_NOTICES.md`. They cover, among other things, wgpu-hal and naga (MIT or Apache-2.0), Pixelify Sans (SIL Open Font License 1.1), Pixelarticons (MIT), Quilt icon artwork (CC0 1.0), and NeoForge icon artwork (CC BY 4.0). Those source files remain in the vendored tree and are included in Tauri bundles where needed.

The CC BY 4.0 NeoForge icon is unmodified; the launcher scales it to display. Attribution and license details are in the Horizon notices. The GNU project's license list identifies CC BY 4.0 as GPL-compatible for artwork and other non-software material; Creative Commons requires appropriate credit, a license link, and an indication of changes when material is shared ([GNU license list](https://www.gnu.org/licenses/license-list.html), [CC BY 4.0 deed](https://creativecommons.org/licenses/by/4.0/)).

## Direct JavaScript dependencies

| Package | License declared by package metadata |
| --- | --- |
| React and React DOM | MIT |
| Lucide React | ISC |
| Tauri JavaScript API and CLI | Apache-2.0 OR MIT |
| Vite and `@vitejs/plugin-react` | MIT |
| TypeScript | Apache-2.0 |
| Vitest | MIT |

The committed npm lockfile records license metadata for all 149 locked packages: MIT (125), Apache-2.0 (3), Apache-2.0 OR MIT (13), ISC (6), BSD-3-Clause (1), and CC BY 4.0 (1). This includes the direct dependencies above and their transitive build/runtime packages. Apache-2.0 is GPLv3-compatible; the GNU project also lists CC BY 4.0 as GPL-compatible for non-software material. See its [license compatibility notes](https://www.gnu.org/licenses/license-list.html).

The build-time `caniuse-lite` browser-compatibility dataset by Ben Briggs and contributors is licensed CC BY 4.0. It is used by the frontend build tooling; it is not application game data. See the [caniuse-lite project](https://github.com/ben-eb/caniuse-lite) and the [CC BY 4.0 deed](https://creativecommons.org/licenses/by/4.0/) for attribution and license information.

## Rust dependencies and review status

The Rust application and Horizon runtime each have a committed `Cargo.lock`. The resolved dependency graphs were checked for x86_64 Linux, Windows GNU, and macOS using `cargo metadata`. The main application has 346, 307, and 312 resolved package records for those targets; the Horizon runtime has 302, 282, and 288. Every resolved package declares license metadata. The expressions include MIT, Apache-2.0 (including the LLVM exception), BSD-2-Clause/3-Clause, ISC, 0BSD, Zlib, BSL-1.0, Unicode-3.0, CC0-1.0, MIT-0, Unlicense, MPL-2.0, and CDLA-Permissive-2.0. No missing license declarations or non-free license expressions were found in these graphs. The GNU project's license list describes the relevant permissive licenses and the GPLv3 compatibility provisions for MPL 2.0; retain source files under their original notices and terms ([GNU license list](https://www.gnu.org/licenses/license-list.html), [Mozilla MPL FAQ](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)). This is a review of declared dependency terms, not a substitute for legal advice. Windows/macOS builds remain unvalidated.

The Horizon runtime's `webpki-roots` dependency incorporates Mozilla CA root data derived from CCADB under CDLA-Permissive-2.0. The agreement applies to this data and requires its text to be available when sharing it. See the [upstream crate attribution](https://github.com/rustls/webpki-roots) and the [official CDLA text](https://cdla.dev/permissive-2-0/). The full agreement text is reproduced below and this notice is included in the desktop bundle.

### Community Data License Agreement — Permissive, Version 2.0

This is the Community Data License Agreement — Permissive, Version 2.0 (the “agreement”). Data Provider(s) and Data Recipient(s) agree as follows:

1. **Provision of the Data**
   1. A Data Recipient may use, modify, and share the Data made available by Data Provider(s) under this agreement if that Data Recipient follows the terms of this agreement.
   2. This agreement does not impose any restriction on a Data Recipient's use, modification, or sharing of any portions of the Data that are in the public domain or that may be used, modified, or shared under any other legal exception or limitation.
2. **Conditions for Sharing Data**
   1. A Data Recipient may share Data, with or without modifications, so long as the Data Recipient makes available the text of this agreement with the shared Data.
3. **No Restrictions on Results**
   1. This agreement does not impose any restriction or obligations with respect to the use, modification, or sharing of Results.
4. **No Warranty; Limitation of Liability**
   1. All Data Recipients receive the Data subject to the following terms:

      THE DATA IS PROVIDED ON AN “AS IS” BASIS, WITHOUT REPRESENTATIONS, WARRANTIES OR CONDITIONS OF ANY KIND, EITHER EXPRESS OR IMPLIED INCLUDING, WITHOUT LIMITATION, ANY WARRANTIES OR CONDITIONS OF TITLE, NON-INFRINGEMENT, MERCHANTABILITY OR FITNESS FOR A PARTICULAR PURPOSE.

      NO DATA PROVIDER SHALL HAVE ANY LIABILITY FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING WITHOUT LIMITATION LOST PROFITS), HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE) ARISING IN ANY WAY OUT OF THE DATA OR RESULTS, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGES.
5. **Definitions**
   1. “Data” means the material received by a Data Recipient under this agreement.
   2. “Data Provider” means any person who is the source of Data provided under this agreement and in reliance on a Data Recipient's agreement to its terms.
   3. “Data Recipient” means any person who receives Data directly or indirectly from a Data Provider and agrees to the terms of this agreement.
   4. “Results” means any outcome obtained by computational analysis of Data, including for example machine learning models and models' insights.

The official version is available at [cdla.dev/permissive-2-0](https://cdla.dev/permissive-2-0/).

The Horizon runtime's `webpki-roots` data dependency is covered by the CDLA-Permissive-2.0 notice above. The AppImage build bundles this file and the project GPL license; review the complete binary notice bundle again before distributing a release artifact.

The Horizon vendored dependency notices are not replaced by this summary. Keep their original license files and attribution text whenever updating the Horizon source.
