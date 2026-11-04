import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, ArrowUpRight, Boxes, Check, CircleAlert, Code2, FilePlus2, LoaderCircle, PackageOpen, RefreshCw, ShieldAlert, Sparkles, Trash2 } from "lucide-react";
import { ModsPage } from "./ModsPage";
import type { HorizonFabricBridgeStatus, HorizonMod, Instance } from "../types";

interface HorizonModsPageProps {
  desktop: boolean;
  instances: Instance[];
  selectedInstanceId: string | null;
  onSelectInstance: (id: string) => void;
  onOpenInstances: () => void;
  onOpenHorizonSettings: () => void;
  curseForgeKey: string;
  onCurseForgeKeyChange: (value: string) => void;
}

type HorizonModsTab = "content" | "fabric";

function formatSize(size: number): string {
  return size >= 1024 * 1024 ? `${(size / (1024 * 1024)).toFixed(1)} MB` : `${Math.max(1, Math.round(size / 1024))} KB`;
}

export function HorizonModsPage({ desktop, instances, selectedInstanceId, onSelectInstance, onOpenInstances, onOpenHorizonSettings, curseForgeKey, onCurseForgeKeyChange }: HorizonModsPageProps) {
  const horizonInstances = instances.filter((instance) => instance.engine === "horizon");
  const selected = horizonInstances.find((instance) => instance.id === selectedInstanceId) ?? horizonInstances[0] ?? null;
  const [tab, setTab] = useState<HorizonModsTab>("content");
  const [mods, setMods] = useState<HorizonMod[]>([]);
  const [path, setPath] = useState("");
  const [loading, setLoading] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState("");
  const [pendingRemove, setPendingRemove] = useState<HorizonMod | null>(null);
  const [bridge, setBridge] = useState<HorizonFabricBridgeStatus | null>(null);
  const [bridgeLoading, setBridgeLoading] = useState(false);
  const [bridgeSaving, setBridgeSaving] = useState(false);
  const [bridgeError, setBridgeError] = useState("");

  async function refresh() {
    if (!selected) { setMods([]); return; }
    setLoading(true);
    setError("");
    try {
      setMods(await invoke<HorizonMod[]>("list_horizon_mods", { instanceId: selected.id }));
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  }

  async function refreshBridge() {
    if (!selected) { setBridge(null); return; }
    setBridgeLoading(true);
    setBridgeError("");
    try {
      setBridge(await invoke<HorizonFabricBridgeStatus>("get_horizon_fabric_bridge_status", { instanceId: selected.id }));
    } catch (reason) {
      setBridgeError(String(reason));
    } finally {
      setBridgeLoading(false);
    }
  }

  useEffect(() => { void refresh(); void refreshBridge(); }, [selected?.id]);

  async function setBridgeEnabled(enabled: boolean) {
    if (!selected || bridgeSaving) return;
    setBridgeSaving(true);
    setBridgeError("");
    try {
      await invoke("set_horizon_fabric_bridge_enabled", { instanceId: selected.id, enabled });
      await refreshBridge();
    } catch (reason) {
      setBridgeError(String(reason));
    } finally {
      setBridgeSaving(false);
    }
  }

  async function setCodeBridgeEnabled(enabled: boolean) {
    if (!selected || bridgeSaving) return;
    setBridgeSaving(true);
    setBridgeError("");
    try {
      await invoke("set_horizon_fabric_code_bridge_enabled", { instanceId: selected.id, enabled });
      await refreshBridge();
    } catch (reason) {
      setBridgeError(String(reason));
    } finally {
      setBridgeSaving(false);
    }
  }

  async function install(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selected || !path.trim()) return;
    setInstalling(true);
    setError("");
    try {
      await invoke("install_horizon_mod", { instanceId: selected.id, sourcePath: path.trim() });
      setPath("");
      await refresh();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setInstalling(false);
    }
  }

  async function toggle(mod: HorizonMod) {
    if (!selected || !mod.valid) return;
    setError("");
    try {
      await invoke("set_horizon_mod_enabled", { instanceId: selected.id, fileName: mod.fileName, enabled: !mod.enabled });
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function remove() {
    if (!selected || !pendingRemove) return;
    setError("");
    try {
      await invoke("remove_horizon_mod", { instanceId: selected.id, fileName: pendingRemove.fileName, confirmed: true });
      setPendingRemove(null);
      await refresh();
    } catch (reason) {
      setError(String(reason));
    }
  }

  return (
    <>
      <section className="page-heading mods-page-heading">
        <div><p className="eyebrow"><PackageOpen size={14} /> HORIZON MOD WORKSPACE</p><h1>Mods for Horizon</h1><p className="page-subtitle">Manage Rust Horizon content or browse code-free Fabric resources. Java Fabric mods use a separate Minecraft Java client.</p></div>
        {horizonInstances.length > 0 && <div className="settings-select-wrap"><select aria-label="Horizon profile for mods" value={selected?.id ?? ""} onChange={(event) => onSelectInstance(event.target.value)}>{horizonInstances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name} · 26.3</option>)}</select></div>}
      </section>
      <div className="horizon-mod-tabs" role="tablist" aria-label="Horizon mod types">
        <button type="button" role="tab" aria-selected={tab === "content"} className={tab === "content" ? "selected" : ""} onClick={() => setTab("content")}><Boxes size={16} /> Horizon content</button>
        <button type="button" role="tab" aria-selected={tab === "fabric"} className={tab === "fabric" ? "selected" : ""} onClick={() => setTab("fabric")}><Sparkles size={16} /> Fabric resource bridge <span className="horizon-experimental-pill">Experimental</span></button>
      </div>
      {!selected ? (
        <section className="empty-library-card empty-library-large"><div className="empty-icon"><Boxes size={23} /></div><div><strong>Create a Horizon profile first</strong><span>Horizon mods are kept separately from Java mods and standard Minecraft instances.</span></div><button className="play-button" type="button" onClick={onOpenInstances}>Open instances</button></section>
      ) : tab === "content" ? (
        <div className="horizon-mods-layout">
          <section className="settings-group horizon-mod-install-card">
            <header><span><strong>Add a Horizon content mod</strong><small>Choose a local <code>.hmod</code> file. It will be copied into this profile and left off until you enable it.</small></span><FilePlus2 size={18} /></header>
            <form className="horizon-path-row" onSubmit={(event) => void install(event)}><input className="field-control" aria-label="Local Horizon mod file" placeholder="Path to a .hmod file" value={path} onChange={(event) => setPath(event.target.value)} /><button className="play-button" type="submit" disabled={!desktop || !path.trim() || installing}>{installing ? <LoaderCircle size={15} className="spin" /> : <FilePlus2 size={15} />}{installing ? "Adding…" : "Add mod"}</button></form>
            <div className="settings-callout"><AlertTriangle size={16} /><span>Fabric, Forge, and other Java <code>.jar</code> mods do not run in Horizon. Native <code>.hnmod</code> code mods are not installed here.</span></div>
          </section>
          <section className="settings-group horizon-installed-mods">
            <header><span><strong>Installed content mods</strong><small>{loading ? "Reading this profile's mod folder…" : `${mods.length} mod${mods.length === 1 ? "" : "s"} in ${selected.name}`}</small></span><button className="settings-quiet-button" type="button" onClick={() => void refresh()} disabled={loading}><RefreshCw size={14} /> Rescan</button></header>
            {error && <p className="settings-inline-error" role="alert"><CircleAlert size={14} /> {error}</p>}
            {loading ? <div className="settings-empty"><LoaderCircle size={18} className="spin" /><span>Scanning Horizon content mods…</span></div> : mods.length === 0 ? <div className="settings-empty"><PackageOpen size={18} /><span>No Horizon content mods are installed in this profile yet.</span></div> : <div className="horizon-mod-list">{mods.map((mod) => <article className="horizon-mod-row" key={mod.fileName}>
              <span className={`horizon-mod-icon ${mod.valid ? "" : "invalid"}`}>{mod.valid ? <Boxes size={17} /> : <CircleAlert size={17} />}</span>
              <span className="horizon-mod-copy"><strong>{mod.name ?? mod.fileName}</strong><small>{mod.valid ? `${mod.id} · ${mod.version ?? "version unknown"} · ${formatSize(mod.size)}` : `${mod.fileName} · manifest could not be read`}</small></span>
              <span className={`horizon-mod-status ${mod.enabled ? "enabled" : ""}`}><span />{mod.enabled ? "Enabled" : "Off"}</span>
              <button className={mod.enabled ? "secondary-button" : "settings-quiet-button"} type="button" disabled={!mod.valid} onClick={() => void toggle(mod)}>{mod.enabled ? "Turn off" : "Turn on"}</button>
              <button className="icon-button danger-icon-button" type="button" aria-label={`Remove ${mod.name ?? mod.fileName}`} onClick={() => setPendingRemove(mod)}><Trash2 size={15} /></button>
            </article>)}</div>}
          </section>
        </div>
      ) : (
        <div className="horizon-fabric-workspace">
          <div className="mods-loader-notice"><Sparkles size={15} /><span><strong>This workspace keeps Rust content and Fabric code experiments separate.</strong> The resource adapter copies supported code-free assets/data. The optional code bridge runs only a checked Java subset beside native Horizon; Java + Fabric settings still launch the separate Java game.</span><button type="button" className="text-link" onClick={onOpenHorizonSettings}>Fabric runtime settings <ArrowUpRight size={14} /></button></div>
          <section className="horizon-fabric-panel">
            <div className="horizon-fabric-panel-heading">
              <span className="horizon-fabric-mark"><Sparkles size={19} /></span>
              <div><p className="eyebrow">OPTIONAL · STARTUP ONLY</p><h2>Fabric resource bridge</h2><p>Browse provider builds tagged for Minecraft 26.3 and Fabric, then adapt supported files when Horizon starts.</p></div>
              <label className={`horizon-bridge-switch ${bridge?.settings.enabled ? "enabled" : ""}`}>
                <span>{bridge?.settings.enabled ? "On" : "Off"}</span>
                <input type="checkbox" role="switch" aria-label="Enable the Horizon Fabric resource bridge" checked={bridge?.settings.enabled ?? false} disabled={!desktop || bridgeLoading || bridgeSaving || !bridge} onChange={(event) => void setBridgeEnabled(event.target.checked)} />
              </label>
            </div>
            <div className="horizon-fabric-explainer"><ShieldAlert size={16} /><span><strong>This does not run Fabric Loader or translate Java code.</strong> It only copies safe <code>assets/</code> and <code>data/</code> from standalone, code-free JARs into Horizon content packages before launch. Mods with Java classes, entrypoints, mixins, unsupported paths, or extra mod dependencies are skipped. Most gameplay and library mods will not work through this bridge.</span></div>
            <div className="horizon-fabric-code-bridge">
              <div className="horizon-fabric-code-heading">
                <span className="horizon-fabric-mark"><Code2 size={18} /></span>
                <div><p className="eyebrow">OPTIONAL · EXPERIMENTAL</p><h3>Fabric code bridge</h3><p>Runs Fabric's <code>START_CLIENT_TICK</code> and <code>END_CLIENT_TICK</code> callbacks beside Horizon, exposes player and movement-input snapshots including vanilla <code>Minecraft.options</code> bindings, and sends bounded sprint, velocity and look-direction actions, system chat, and HUD text to the Rust game.</p></div>
                <label className={`horizon-bridge-switch ${bridge?.settings.codeEnabled ? "enabled" : ""}`}>
                  <span>{bridge?.settings.codeEnabled ? "On" : "Off"}</span>
                  <input type="checkbox" role="switch" aria-label="Enable the experimental Fabric code bridge" checked={bridge?.settings.codeEnabled ?? false} disabled={!desktop || bridgeLoading || bridgeSaving || !bridge} onChange={(event) => void setCodeBridgeEnabled(event.target.checked)} />
                </label>
              </div>
              <div className="settings-callout horizon-fabric-code-warning"><ShieldAlert size={15} /><span><strong>Mapped calls: player position/velocity/look-direction/health snapshots, world-clock snapshots, read-only vanilla movement state and press edges, sprint control, bounded velocity and local look-direction setters, plain system chat, and simple HUD text.</strong> Movement fields follow Horizon's mapped vanilla actions while playing. Sprint, velocity and look requests are applied by the Rust game thread during active play. Velocity is capped at 4 blocks per tick; yaw is limited to ±360,000° and pitch to −90° through 90°. Look changes affect the local player only. System chat is plain text and follows Horizon's chat visibility setting. The Java Options facade cannot edit bindings or provide conflict contexts; mouse/controller input, world or other-entity access, rendering, server-authoritative changes, and most Minecraft or Fabric APIs are unsupported. Mixins, access wideners, extra mod dependencies, and nested JARs are rejected and excluded from the helper JVM. This does not translate Sodium or arbitrary gameplay changes into Rust. Java code still runs with your user permissions and can access files or the network. This setting is off by default.</span></div>
              {bridge && bridge.codeMods.length > 0 && <div className="horizon-bridge-mod-list" aria-label="Fabric code compatibility scan">{bridge.codeMods.map((mod) => <article className="horizon-bridge-mod-row" key={`code-${mod.fileName}`}>
                <span className={`horizon-bridge-mod-icon ${mod.state === "supported" || mod.state === "api-shim" ? "compatible" : mod.state}`}><Code2 size={16} /></span>
                <span className="horizon-bridge-mod-copy"><strong>{mod.name ?? mod.fileName}</strong><small>{mod.modId ? `${mod.modId}${mod.version ? ` · ${mod.version}` : ""} · ` : ""}{mod.fileName}</small></span>
                <span className={`horizon-bridge-mod-state ${mod.state === "supported" || mod.state === "api-shim" ? "compatible" : mod.state}`}>{mod.state === "supported" ? "Mapped API subset" : mod.state === "api-shim" ? "API shim" : mod.state === "disabled" ? "Disabled" : mod.state === "invalid" ? "Could not inspect" : "Rejected"}</span>
                <span className="horizon-bridge-mod-detail">{mod.detail}</span>
              </article>)}</div>}
            </div>
            <div className="horizon-bridge-summary">
              <span className={`horizon-bridge-state ${bridge?.settings.enabled ? "on" : ""}`}><span />{bridgeLoading ? "Scanning JARs…" : bridge?.settings.enabled ? "Will prepare at next launch" : "Disabled for this profile"}</span>
              <span><Check size={14} /> {bridge?.codeCompatibleCount ?? 0} mapped API candidate{bridge?.codeCompatibleCount === 1 ? "" : "s"}</span>
              <span><CircleAlert size={14} /> {bridge?.codeSkippedCount ?? 0} rejected or unsupported</span>
              <button className="settings-quiet-button" type="button" onClick={() => void refreshBridge()} disabled={bridgeLoading}><RefreshCw size={13} /> Rescan</button>
            </div>
            {bridgeError && <p className="settings-inline-error" role="alert"><CircleAlert size={14} /> {bridgeError}</p>}
            {bridge && bridge.mods.length > 0 && <div className="horizon-bridge-mod-list">{bridge.mods.map((mod) => <article className="horizon-bridge-mod-row" key={mod.fileName}>
              <span className={`horizon-bridge-mod-icon ${mod.state}`}><PackageOpen size={16} /></span>
              <span className="horizon-bridge-mod-copy"><strong>{mod.name ?? mod.fileName}</strong><small>{mod.modId ? `${mod.modId}${mod.version ? ` · ${mod.version}` : ""} · ` : ""}{mod.fileName}</small></span>
              <span className={`horizon-bridge-mod-state ${mod.state}`}>{mod.state === "compatible" ? "Static content" : mod.state === "disabled" ? "JAR disabled" : mod.state === "invalid" ? "Could not inspect" : "Skipped"}</span>
              <span className="horizon-bridge-mod-detail">{mod.detail}</span>
            </article>)}</div>}
            {!bridgeLoading && bridge?.mods.length === 0 && <div className="horizon-bridge-empty"><PackageOpen size={18} /><span>No Fabric JARs in this profile yet. Install a project below to check its files.</span></div>}
          </section>
          <ModsPage desktop={desktop} instances={horizonInstances} selectedInstanceId={selected.id} onSelectInstance={onSelectInstance} onOpenInstances={onOpenInstances} curseForgeKey={curseForgeKey} onCurseForgeKeyChange={onCurseForgeKeyChange} hidePageHeading hideInstancePicker onModsChanged={() => void refreshBridge()} />
        </div>
      )}
      {pendingRemove && <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setPendingRemove(null); }}><section className="modal-card small-modal" role="alertdialog" aria-modal="true" aria-labelledby="horizon-mod-remove-title"><div className="modal-header"><div><p className="eyebrow danger-eyebrow"><CircleAlert size={14} /> REMOVE MOD</p><h2 id="horizon-mod-remove-title">Remove {pendingRemove.name ?? pendingRemove.fileName}?</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setPendingRemove(null)}>×</button></div><p className="modal-copy">This removes the selected <code>.hmod</code> file from {selected?.name}. Its world and other profile files stay in place.</p><div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setPendingRemove(null)}>Keep mod</button><button className="danger-button" type="button" onClick={() => void remove()}><Trash2 size={14} /> Remove mod</button></div></section></div>}
    </>
  );
}
