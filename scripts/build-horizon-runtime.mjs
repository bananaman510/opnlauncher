import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, chmodSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const profile = process.argv[2] === "release" ? "release" : "debug";
const target = process.env.TAURI_ENV_TARGET_TRIPLE;
const targetDir = resolve(projectRoot, "target/horizon-runtime");
const runtimeManifest = resolve(projectRoot, "src-tauri/horizon-runtime/Cargo.toml");
const windowsTarget = target ? target.includes("windows") : process.platform === "win32";
const outputName = windowsTarget ? "horizon-runtime.exe" : "horizon-runtime";
const args = ["build", "--manifest-path", runtimeManifest, "--target-dir", targetDir, "--bin", "horizon-runtime"];
if (profile === "release") args.push("--release");
if (target) args.push("--target", target);
if (process.platform === "linux") {
  const hasAlsa = spawnSync("pkg-config", ["--exists", "alsa"], { stdio: "ignore" }).status === 0;
  const hasLibudev = spawnSync("pkg-config", ["--exists", "libudev"], { stdio: "ignore" }).status === 0;
  args.push("--no-default-features");
  if (hasAlsa && hasLibudev) {
    args.push("--features", "audio,controllers");
  } else {
    const missing = [!hasAlsa && "alsa", !hasLibudev && "libudev"].filter(Boolean);
    console.warn(`Horizon audio or controller support is disabled; install the development packages for ${missing.join(" and ")} and rebuild to include it.`);
  }
}

const result = spawnSync(process.env.CARGO ?? "cargo", args, {
  cwd: projectRoot,
  stdio: "inherit",
  env: { ...process.env, HORIZON_TELEMETRY_URL: "off", CCACHE_DISABLE: "1" },
});
if (result.error) throw result.error;
if (result.status !== 0) process.exit(result.status ?? 1);

const targetSegment = target ? `${target}/` : "";
const builtRuntime = resolve(targetDir, targetSegment, profile, outputName);
const bundledRuntime = resolve(projectRoot, "src-tauri/binaries/horizon-runtime.exe");
mkdirSync(dirname(bundledRuntime), { recursive: true });
copyFileSync(builtRuntime, bundledRuntime);
if (process.platform !== "win32") chmodSync(bundledRuntime, 0o755);
console.log(`Built Horizon runtime at ${bundledRuntime}`);
