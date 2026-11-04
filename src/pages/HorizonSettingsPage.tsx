import { ArrowDownToLine, Check, CircleAlert, Cpu, ExternalLink, FolderOpen, Gauge, HardDrive, Info, LoaderCircle, Plus, RefreshCw, ShieldAlert, ShieldCheck, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DownloadProgress, HorizonClientInfo, Instance, JavaRuntime, LoaderVersion, MinecraftInstallation } from "../types";

interface HorizonSettingsPageProps {
  instances: Instance[];
  selectedInstance: Instance | null;
  onSelectInstance: (id: string) => void;
  resourcesPath: string;
  resourcesReady: boolean;
  prepareJavaPath: string;
  onPrepareJavaPathChange: (path: string) => void;
  runtimes: JavaRuntime[];
  installations: MinecraftInstallation[];
  minecraftSourcePath: string;
  onMinecraftSourceChange: (path: string) => void;
  onAddMinecraftFolder: (path: string) => Promise<void>;
  onScanDevice: () => void;
  scanning: boolean;
  onPrepareResources: () => void;
  preparing: boolean;
  progress: DownloadProgress | null;
  clientInfo: HorizonClientInfo | null;
  error: string;
  width: number;
  height: number;
  fullscreen: boolean;
  onWindowChange: (value: { width: number; height: number; fullscreen: boolean }) => void;
  onInstanceUpdated: (instance: Instance) => void;
  onOpenMods: () => void;
  desktop: boolean;
}

export function HorizonSettingsPage(props: HorizonSettingsPageProps) {
  const [minecraftFolderInput, setMinecraftFolderInput] = useState("");
  const [minecraftFolderError, setMinecraftFolderError] = useState("");
  const horizonInstances = props.instances.filter((instance) => instance.engine === "horizon");
  const horizonInstance = props.selectedInstance?.engine === "horizon" ? props.selectedInstance : null;
  const compatibleInstallations = props.installations.filter((installation) => installation.versions.some((version) => version.id === "26.3"));
  const [fabricVersions, setFabricVersions] = useState<LoaderVersion[]>([]);
  const [fabricVersionsLoading, setFabricVersionsLoading] = useState(false);
  const [fabricSetupError, setFabricSetupError] = useState("");
  const [fabricSetupBusy, setFabricSetupBusy] = useState(false);
  const [fabricJavaPath, setFabricJavaPath] = useState("");
  const [fabricLoaderVersion, setFabricLoaderVersion] = useState("");
  const [fabricDownloadId, setFabricDownloadId] = useState<string | null>(null);
  const [fabricProgress, setFabricProgress] = useState<DownloadProgress | null>(null);

  useEffect(() => {
    if (!horizonInstance || !props.desktop) {
      setFabricVersions([]);
      setFabricVersionsLoading(false);
      return;
    }
    let active = true;
    setFabricVersionsLoading(true);
    setFabricSetupError("");
    setFabricJavaPath(horizonInstance.horizonFabricJavaPath ?? props.runtimes.find((runtime) => runtime.majorVersion >= 25)?.path ?? "");
    setFabricLoaderVersion("");
    void invoke<LoaderVersion[]>("list_loader_versions", { loader: "fabric", gameVersion: "26.3" })
      .then((versions) => {
        if (!active) return;
        setFabricVersions(versions);
        const installedVersion = horizonInstance.horizonFabricVersionId?.match(/^fabric-loader-(.+)-26[.]3$/)?.[1];
        setFabricLoaderVersion(versions.some((version) => version.version === installedVersion)
          ? installedVersion ?? ""
          : versions.find((version) => version.stable)?.version ?? versions[0]?.version ?? "");
      })
      .catch((reason: unknown) => { if (active) setFabricSetupError(String(reason)); })
      .finally(() => { if (active) setFabricVersionsLoading(false); });
    return () => { active = false; };
  }, [horizonInstance?.id, props.desktop, props.runtimes]);

  useEffect(() => {
    if (!fabricDownloadId) return;
    let unlisten: UnlistenFn | undefined;
    let stopped = false;
    void listen<DownloadProgress>("download-progress", (event) => {
      if (event.payload.downloadId === fabricDownloadId) setFabricProgress(event.payload);
    }).then((dispose) => { if (stopped) dispose(); else unlisten = dispose; });
    return () => { stopped = true; unlisten?.(); };
  }, [fabricDownloadId]);

  async function installFabricRuntime() {
    if (!horizonInstance || !fabricJavaPath || !fabricLoaderVersion || fabricSetupBusy) return;
    const downloadId = crypto.randomUUID();
    setFabricDownloadId(downloadId);
    setFabricProgress(null);
    setFabricSetupError("");
    setFabricSetupBusy(true);
    try {
      const updated = await invoke<Instance>("install_horizon_fabric_setup", {
        instanceId: horizonInstance.id,
        loaderVersion: fabricLoaderVersion,
        javaPath: fabricJavaPath,
        downloadId,
      });
      props.onInstanceUpdated(updated);
      setFabricProgress((current) => current ? { ...current, phase: "Fabric setup complete" } : current);
    } catch (reason) {
      setFabricSetupError(String(reason));
    } finally {
      setFabricSetupBusy(false);
      setFabricDownloadId(null);
    }
  }

  async function cancelFabricSetup() {
    if (!fabricDownloadId) return;
    try { await invoke("cancel_download", { downloadId: fabricDownloadId }); }
    catch (reason) { setFabricSetupError(String(reason)); }
  }

  async function setFabricRuntimeEnabled(enabled: boolean) {
    if (!horizonInstance || fabricSetupBusy) return;
    setFabricSetupError("");
    try {
      await invoke("set_horizon_fabric_runtime_enabled", { instanceId: horizonInstance.id, enabled });
      props.onInstanceUpdated({ ...horizonInstance, horizonFabricEnabled: enabled });
    } catch (reason) {
      setFabricSetupError(String(reason));
    }
  }

  async function addMinecraftFolder() {
    setMinecraftFolderError("");
    try {
      await props.onAddMinecraftFolder(minecraftFolderInput);
      setMinecraftFolderInput("");
    } catch (reason) {
      setMinecraftFolderError(String(reason));
    }
  }
  return (
    <>
      <section className="page-heading settings-page-heading horizon-page-heading">
        <div><p className="eyebrow"><Sparkles size={14} /> RUST GAME ENGINE</p><h1>Horizon</h1><p className="page-subtitle">Manage the separate Minecraft 26.3 Rust profiles in OpnLauncher.</p></div>
        {horizonInstances.length > 0 && <div className="settings-select-wrap horizon-instance-picker"><select aria-label="Horizon instance" value={props.selectedInstance?.engine === "horizon" ? props.selectedInstance.id : ""} onChange={(event) => props.onSelectInstance(event.target.value)}><option value="" disabled>Select a Horizon profile</option>{horizonInstances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name}</option>)}</select></div>}
      </section>

      <div className="settings-workspace horizon-settings-workspace">
        <main className="settings-content horizon-settings-content">
          <section className="settings-section-view horizon-runtime-layout">
            <div className="settings-section-heading"><h2>Horizon runtime</h2><p>OpnLauncher builds and manages Horizon directly from its included Rust source.</p></div>
            <div className="settings-group horizon-engine-card">
              <header><span><strong>Integrated Horizon engine</strong><small>Game client source is built into OpnLauncher as a managed runtime.</small></span><Cpu size={17} /></header>
              {props.clientInfo ? <div className="horizon-verified"><Check size={15} /><span><strong>Horizon {props.clientInfo.version} is included</strong><small>Minecraft {props.clientInfo.minecraftVersion} · Local profiles · telemetry disabled</small></span></div> : <p className="horizon-path-help"><CircleAlert size={14} /> The bundled Horizon runtime is not available in this build. Rebuild OpnLauncher to include it.</p>}
            </div>
            <div className="settings-group horizon-source-group">
              <header><span><strong>Find installed Minecraft files</strong><small>Search standard folders and common third-party launcher instances.</small></span><FolderOpen size={17} /></header>
              <div className="horizon-source-heading"><span>{props.installations.length} installation{props.installations.length === 1 ? "" : "s"} found</span><button className="settings-quiet-button" type="button" onClick={props.onScanDevice} disabled={!props.desktop || props.scanning}>{props.scanning ? <RefreshCw size={14} className="spin" /> : <RefreshCw size={14} />}{props.scanning ? "Searching…" : "Search this device"}</button></div>
              <label className="field-label" htmlFor="horizon-minecraft-source">Minecraft 26.3 files</label>
              <div className="select-wrap"><select id="horizon-minecraft-source" className="field-control" value={props.minecraftSourcePath} onChange={(event) => props.onMinecraftSourceChange(event.target.value)}><option value="">Download into OpnLauncher’s private copy</option>{compatibleInstallations.map((installation) => <option key={installation.root} value={installation.root}>{installation.root}</option>)}</select><HardDrive size={15} /></div>
              {compatibleInstallations.length === 0 && <p className="horizon-path-help"><Info size={14} /> No detected Minecraft 26.3 installation yet. You can add a folder below, or OpnLauncher will fetch the official files itself.</p>}
              {compatibleInstallations.length > 0 && <p className="horizon-path-help"><ShieldCheck size={14} /> Matching files are checked against Mojang’s published checksums and copied into Horizon’s private cache. The selected Minecraft installation is never changed; missing files are downloaded.</p>}
              <div className="horizon-path-row"><input className="field-control" aria-label="Add Minecraft folder for Horizon" placeholder="Add another .minecraft or launcher instance folder" value={minecraftFolderInput} onChange={(event) => setMinecraftFolderInput(event.target.value)} /><button className="secondary-button" type="button" onClick={() => void addMinecraftFolder()} disabled={!props.desktop || !minecraftFolderInput.trim()}><Plus size={14} /> Add folder</button></div>
              {minecraftFolderError && <p className="settings-inline-error" role="alert"><CircleAlert size={14} /> {minecraftFolderError}</p>}
            </div>
            <div className="settings-group horizon-resources-card">
              <header><span><strong>Prepared game resources</strong><small>Managed automatically from your own Minecraft 26.3 files.</small></span><FolderOpen size={17} /></header>
              <div className="horizon-verified">{props.resourcesReady ? <Check size={15} /> : <Info size={15} />}<span><strong>{props.resourcesReady ? "Resources ready" : "Resources not prepared"}</strong><small>{props.resourcesPath || "OpnLauncher’s private Horizon folder"}</small></span></div>
              <p className="horizon-path-help"><HardDrive size={14} /> The resource folder is managed by OpnLauncher. Worlds, local settings, and Horizon mods stay in each profile's own folder.</p>
            </div>
            <div className="settings-group horizon-prepare-group">
              <header><span><strong>Download and prepare Minecraft 26.3</strong><small>Fetch the official client, libraries, assets, and server file, then let Horizon prepare its Rust resources.</small></span><ArrowDownToLine size={17} /></header>
              <label className="field-label" htmlFor="horizon-prepare-java">JDK 25 for preparation</label>
              <div className="select-wrap"><select id="horizon-prepare-java" className="field-control" value={props.prepareJavaPath} onChange={(event) => props.onPrepareJavaPathChange(event.target.value)}><option value="">Choose an installed Java 25 runtime</option>{props.runtimes.filter((runtime) => runtime.majorVersion === 25).map((runtime) => <option key={runtime.path} value={runtime.path}>{runtime.version} · {runtime.vendor ?? "JDK"}</option>)}</select><Gauge size={15} /></div>
              {props.runtimes.every((runtime) => runtime.majorVersion !== 25) && <p className="horizon-path-help"><Info size={14} /> Install JDK 25 and add its Java executable under Settings → Java Manager. Resource preparation needs a JDK, including <code>javac</code>.</p>}
              <button className="play-button horizon-prepare-button" type="button" disabled={!props.desktop || !props.prepareJavaPath || props.preparing} onClick={props.onPrepareResources}>{props.preparing ? <LoaderCircle size={16} className="spin" /> : <ArrowDownToLine size={16} />}{props.preparing ? "Preparing Horizon resources…" : "Download files & prepare Horizon"}</button>
              {props.preparing && props.progress?.versionId.includes("Horizon") && <div className="install-progress-card"><div><LoaderCircle className="spin" size={16} /><strong>{props.progress.phase}</strong><span>{props.progress.totalFiles ? `${props.progress.completedFiles} / ${props.progress.totalFiles} steps` : "This may take several minutes"}</span></div><div className="download-progress-track"><span style={{ width: `${props.progress.totalBytes ? Math.min(100, Math.round(props.progress.downloadedBytes / props.progress.totalBytes * 100)) : props.progress.totalFiles ? Math.min(100, Math.round(props.progress.completedFiles / props.progress.totalFiles * 100)) : 0}%` }} /></div>{props.progress.currentFile && <small>{props.progress.currentFile}</small>}</div>}
              <p className="horizon-path-help"><ShieldCheck size={14} /> Official game files are fetched from Mojang over HTTPS and verified against their published checksums. They stay in OpnLauncher’s local Horizon support folder; no Microsoft account or session is used.</p>
            </div>
            {props.error && <p className="settings-inline-error" role="alert"><CircleAlert size={14} /> {props.error}</p>}
            <div className="settings-callout"><ShieldCheck size={16} /><span>Horizon starts with your local, unverified profile. Microsoft sign-in is not configured, no account token is passed, and Horizon telemetry and its “playing” ping are disabled for these launches.</span></div>
          </section>

          <section className="settings-section-view">
            <div className="settings-section-heading"><h2>Selected profile settings</h2><p>These window settings are kept separately for each Horizon instance.</p></div>
            {horizonInstance ? (
              <div className="settings-group horizon-window-settings">
                <div className="horizon-window-size"><label><span>Width</span><input className="field-control" aria-label="Horizon window width" type="number" min={640} max={8192} step={80} value={props.width} onChange={(event) => props.onWindowChange({ width: Number(event.target.value), height: props.height, fullscreen: props.fullscreen })} /></label><span aria-hidden="true">×</span><label><span>Height</span><input className="field-control" aria-label="Horizon window height" type="number" min={480} max={8192} step={80} value={props.height} onChange={(event) => props.onWindowChange({ width: props.width, height: Number(event.target.value), fullscreen: props.fullscreen })} /></label></div>
                <label className="horizon-fullscreen"><span><strong>Start fullscreen</strong><small>Use the selected profile's preference when Horizon opens.</small></span><input type="checkbox" checked={props.fullscreen} onChange={(event) => props.onWindowChange({ width: props.width, height: props.height, fullscreen: event.target.checked })} /></label>
                <div className="horizon-path-help"><Gauge size={14} /> Horizon is a native Rust game, so Java and Fabric/Forge settings do not apply.</div>
                <section className="horizon-java-compat-card" aria-labelledby="horizon-java-compat-title">
                  <div className="horizon-java-compat-heading"><span className="horizon-fabric-mark"><Sparkles size={18} /></span><div><p className="eyebrow">OPTIONAL · EXPERIMENTAL</p><h3 id="horizon-java-compat-title">Separate Java Minecraft + Fabric client</h3><p>Runs a private Minecraft Java 26.3 client with Fabric and unchanged Fabric JARs.</p></div></div>
                  <div className="horizon-fabric-runtime-notice"><ShieldAlert size={16} /><span>This does not add Fabric support to the Rust Horizon engine or translate Java into Rust. When enabled, Play starts this separate Java Minecraft client. Switch it off to play Horizon Rust again. The clients keep separate worlds and mods, and individual Fabric mods may still be incompatible.</span></div>
                  <label className={`horizon-fullscreen horizon-java-runtime-toggle ${horizonInstance.horizonFabricEnabled ? "is-enabled" : ""}`}><span><strong>Use the separate Java + Fabric client</strong><small>{horizonInstance.horizonFabricEnabled ? `Enabled · Play starts ${horizonInstance.horizonFabricVersionId ?? "the Java Fabric client"}` : horizonInstance.horizonFabricVersionId ? `Installed · ${horizonInstance.horizonFabricVersionId} · currently off` : "Off · Play starts the Rust Horizon client"}</small></span><input type="checkbox" role="switch" aria-label="Use the separate Java Fabric client for this Horizon profile" checked={horizonInstance.horizonFabricEnabled} disabled={!props.desktop || fabricSetupBusy || !horizonInstance.horizonFabricVersionId} onChange={(event) => void setFabricRuntimeEnabled(event.target.checked)} /></label>
                  <div className="horizon-java-runtime-controls">
                    <label><span>Java 25 runtime</span><span className="select-wrap"><select className="field-control" aria-label="Java runtime for Horizon Fabric" value={fabricJavaPath} onChange={(event) => setFabricJavaPath(event.target.value)}><option value="">Choose an installed Java 25 runtime</option>{props.runtimes.filter((runtime) => runtime.majorVersion >= 25).map((runtime) => <option key={runtime.path} value={runtime.path}>{runtime.version} · {runtime.vendor ?? "JDK"}</option>)}</select></span></label>
                    <label><span>Fabric Loader build for Minecraft 26.3</span><span className="select-wrap"><select className="field-control" aria-label="Fabric Loader version for Horizon" value={fabricLoaderVersion} disabled={fabricVersionsLoading || fabricVersions.length === 0} onChange={(event) => setFabricLoaderVersion(event.target.value)}><option value="">{fabricVersionsLoading ? "Loading Fabric builds…" : "Choose a Fabric build"}</option>{fabricVersions.map((version) => <option key={version.version} value={version.version}>{version.version}{version.stable ? " · stable" : ""}</option>)}</select></span></label>
                  </div>
                  <div className="horizon-java-runtime-actions"><button className="secondary-button" type="button" disabled={!props.desktop || fabricSetupBusy || !fabricJavaPath || !fabricLoaderVersion} onClick={() => void installFabricRuntime()}>{fabricSetupBusy ? <LoaderCircle size={15} className="spin" /> : <ArrowDownToLine size={15} />}{fabricSetupBusy ? "Preparing Java + Fabric…" : horizonInstance.horizonFabricVersionId ? "Update Java + Fabric files" : "Install Java + Fabric files"}</button>{horizonInstance.horizonFabricEnabled && <button className="settings-quiet-button" type="button" onClick={props.onOpenMods}><ExternalLink size={14} /> Manage Fabric mods</button>}{fabricSetupBusy && <button className="settings-quiet-button" type="button" onClick={() => void cancelFabricSetup()}>Cancel</button>}</div>
                  {fabricSetupBusy && fabricProgress && <div className="install-progress-card"><div><LoaderCircle className="spin" size={16} /><strong>{fabricProgress.phase}</strong><span>{fabricProgress.totalFiles ? `${fabricProgress.completedFiles} / ${fabricProgress.totalFiles} files` : "Preparing isolated files"}</span></div><div className="download-progress-track"><span style={{ width: `${fabricProgress.totalFiles ? Math.min(100, Math.round(fabricProgress.completedFiles / fabricProgress.totalFiles * 100)) : 0}%` }} /></div>{fabricProgress.currentFile && <small>{fabricProgress.currentFile}</small>}</div>}
                  {fabricSetupError && <p className="settings-inline-error" role="alert"><CircleAlert size={14} /> {fabricSetupError}</p>}
                  {props.runtimes.every((runtime) => runtime.majorVersion < 25) && <p className="horizon-path-help"><Info size={14} /> Add a Java 25 or newer runtime in Settings → Java Manager. This profile keeps its Java files, libraries, and mods in a private folder.</p>}
                </section>
              </div>
            ) : <div className="settings-empty"><Info size={18} /><span>Select a Horizon profile to edit its window settings.</span></div>}
          </section>
        </main>
      </div>
    </>
  );
}
