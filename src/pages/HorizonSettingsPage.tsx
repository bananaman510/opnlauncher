import { ArrowDownToLine, Check, CircleAlert, Cpu, FolderOpen, Gauge, HardDrive, Info, LoaderCircle, RefreshCw, ShieldCheck, Sparkles } from "lucide-react";
import type { DownloadProgress, HorizonClientInfo, Instance, JavaRuntime } from "../types";

interface HorizonSettingsPageProps {
  instances: Instance[];
  selectedInstance: Instance | null;
  onSelectInstance: (id: string) => void;
  clientPath: string;
  onClientPathChange: (path: string) => void;
  resourcesPath: string;
  onResourcesPathChange: (path: string) => void;
  preparerPath: string;
  onPreparerPathChange: (path: string) => void;
  prepareJavaPath: string;
  onPrepareJavaPathChange: (path: string) => void;
  runtimes: JavaRuntime[];
  onPrepareResources: () => void;
  preparing: boolean;
  progress: DownloadProgress | null;
  clientInfo: HorizonClientInfo | null;
  onInspectClient: () => void;
  inspecting: boolean;
  error: string;
  width: number;
  height: number;
  fullscreen: boolean;
  onWindowChange: (value: { width: number; height: number; fullscreen: boolean }) => void;
  desktop: boolean;
}

export function HorizonSettingsPage(props: HorizonSettingsPageProps) {
  const horizonInstances = props.instances.filter((instance) => instance.engine === "horizon");
  return (
    <>
      <section className="page-heading settings-page-heading">
        <div><p className="eyebrow"><Sparkles size={14} /> RUST GAME ENGINE</p><h1>Horizon</h1><p className="page-subtitle">Manage the separate Minecraft 26.3 Rust profiles in OpnLauncher.</p></div>
        {horizonInstances.length > 0 && <div className="settings-select-wrap horizon-instance-picker"><select aria-label="Horizon instance" value={props.selectedInstance?.engine === "horizon" ? props.selectedInstance.id : ""} onChange={(event) => props.onSelectInstance(event.target.value)}><option value="" disabled>Select a Horizon profile</option>{horizonInstances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name}</option>)}</select></div>}
      </section>

      <div className="settings-workspace horizon-settings-workspace">
        <main className="settings-content horizon-settings-content">
          <section className="settings-section-view">
            <div className="settings-section-heading"><h2>Horizon runtime</h2><p>Connect OpnLauncher to the local Horizon client and its prepared game files.</p></div>
            <div className="settings-group">
              <header><span><strong>Horizon client</strong><small>Path to the built <code>horizon-client</code> executable.</small></span><Cpu size={17} /></header>
              <div className="horizon-path-row"><input className="field-control" aria-label="Horizon client executable" placeholder="/path/to/horizon-client" value={props.clientPath} onChange={(event) => props.onClientPathChange(event.target.value)} /><button className="secondary-button" type="button" onClick={props.onInspectClient} disabled={!props.desktop || !props.clientPath.trim() || props.inspecting}>{props.inspecting ? <RefreshCw size={14} className="spin" /> : <Check size={14} />}{props.inspecting ? "Checking…" : "Verify client"}</button></div>
              {props.clientInfo && <div className="horizon-verified"><Check size={15} /><span><strong>Horizon {props.clientInfo.version}</strong><small>Minecraft {props.clientInfo.minecraftVersion} · Launch interface {props.clientInfo.launchApi}</small></span></div>}
            </div>
            <div className="settings-group">
              <header><span><strong>Prepared game resources</strong><small>Folder produced by <code>horizon-prepare</code> from Minecraft 26.3 files.</small></span><FolderOpen size={17} /></header>
              <div className="horizon-path-row"><input className="field-control" aria-label="Prepared Horizon resources folder" placeholder="/path/to/prepared-horizon-resources" value={props.resourcesPath} onChange={(event) => props.onResourcesPathChange(event.target.value)} onBlur={() => props.onResourcesPathChange(props.resourcesPath.trim())} /></div>
              <p className="horizon-path-help"><HardDrive size={14} /> OpnLauncher uses this shared resource folder for Horizon profiles. Worlds, local settings, and mods stay in each profile's own folder.</p>
            </div>
            <div className="settings-group horizon-prepare-group">
              <header><span><strong>Download and prepare Minecraft 26.3</strong><small>Fetch the official client, libraries, assets, and server file, then let Horizon prepare its Rust resources.</small></span><ArrowDownToLine size={17} /></header>
              <label className="field-label" htmlFor="horizon-preparer-path">Horizon resource preparer</label>
              <input id="horizon-preparer-path" className="field-control" aria-label="Horizon resource preparer executable" placeholder="Path to horizon-prepare" value={props.preparerPath} onChange={(event) => props.onPreparerPathChange(event.target.value)} />
              <label className="field-label" htmlFor="horizon-prepare-java">JDK 25 for preparation</label>
              <div className="select-wrap"><select id="horizon-prepare-java" className="field-control" value={props.prepareJavaPath} onChange={(event) => props.onPrepareJavaPathChange(event.target.value)}><option value="">Choose an installed Java 25 runtime</option>{props.runtimes.filter((runtime) => runtime.majorVersion === 25).map((runtime) => <option key={runtime.path} value={runtime.path}>{runtime.version} · {runtime.vendor ?? "JDK"}</option>)}</select><Gauge size={15} /></div>
              {props.runtimes.every((runtime) => runtime.majorVersion !== 25) && <p className="horizon-path-help"><Info size={14} /> Install JDK 25 and add its Java executable under Settings → Java Manager. Resource preparation needs a JDK, including <code>javac</code>.</p>}
              <button className="play-button horizon-prepare-button" type="button" disabled={!props.desktop || !props.preparerPath.trim() || !props.prepareJavaPath || props.preparing} onClick={props.onPrepareResources}>{props.preparing ? <LoaderCircle size={16} className="spin" /> : <ArrowDownToLine size={16} />}{props.preparing ? "Preparing Horizon resources…" : "Download files & prepare Horizon"}</button>
              {props.preparing && props.progress?.versionId.includes("Horizon") && <div className="install-progress-card"><div><LoaderCircle className="spin" size={16} /><strong>{props.progress.phase}</strong><span>{props.progress.totalFiles ? `${props.progress.completedFiles} / ${props.progress.totalFiles} files` : "This may take several minutes"}</span></div><div className="download-progress-track"><span style={{ width: `${props.progress.totalBytes ? Math.min(100, Math.round(props.progress.downloadedBytes / props.progress.totalBytes * 100)) : 0}%` }} /></div>{props.progress.currentFile && <small>{props.progress.currentFile}</small>}</div>}
              <p className="horizon-path-help"><ShieldCheck size={14} /> Official game files are fetched from Mojang over HTTPS and verified against their published checksums. They stay in OpnLauncher’s local Horizon support folder; no Microsoft account or session is used.</p>
            </div>
            {props.error && <p className="settings-inline-error" role="alert"><CircleAlert size={14} /> {props.error}</p>}
            <div className="settings-callout"><ShieldCheck size={16} /><span>Horizon starts with your local, unverified profile. Microsoft sign-in is not configured, no account token is passed, and Horizon telemetry and its “playing” ping are disabled for these launches.</span></div>
          </section>

          <section className="settings-section-view">
            <div className="settings-section-heading"><h2>Selected profile settings</h2><p>These window settings are kept separately for each Horizon instance.</p></div>
            {props.selectedInstance?.engine === "horizon" ? (
              <div className="settings-group horizon-window-settings">
                <div className="horizon-window-size"><label><span>Width</span><input className="field-control" aria-label="Horizon window width" type="number" min={640} max={8192} step={80} value={props.width} onChange={(event) => props.onWindowChange({ width: Number(event.target.value), height: props.height, fullscreen: props.fullscreen })} /></label><span aria-hidden="true">×</span><label><span>Height</span><input className="field-control" aria-label="Horizon window height" type="number" min={480} max={8192} step={80} value={props.height} onChange={(event) => props.onWindowChange({ width: props.width, height: Number(event.target.value), fullscreen: props.fullscreen })} /></label></div>
                <label className="horizon-fullscreen"><span><strong>Start fullscreen</strong><small>Use the selected profile's preference when Horizon opens.</small></span><input type="checkbox" checked={props.fullscreen} onChange={(event) => props.onWindowChange({ width: props.width, height: props.height, fullscreen: event.target.checked })} /></label>
                <div className="horizon-path-help"><Gauge size={14} /> Horizon is a native Rust game, so Java and Fabric/Forge settings do not apply.</div>
              </div>
            ) : <div className="settings-empty"><Info size={18} /><span>Select a Horizon profile to edit its window settings.</span></div>}
          </section>
        </main>
      </div>
    </>
  );
}
