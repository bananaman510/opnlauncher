import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Boxes, CircleAlert, FilePlus2, LoaderCircle, PackageOpen, RefreshCw, Trash2 } from "lucide-react";
import type { HorizonMod, Instance } from "../types";

interface HorizonModsPageProps {
  desktop: boolean;
  instances: Instance[];
  selectedInstanceId: string | null;
  onSelectInstance: (id: string) => void;
  onOpenInstances: () => void;
}

function formatSize(size: number): string {
  return size >= 1024 * 1024 ? `${(size / (1024 * 1024)).toFixed(1)} MB` : `${Math.max(1, Math.round(size / 1024))} KB`;
}

export function HorizonModsPage({ desktop, instances, selectedInstanceId, onSelectInstance, onOpenInstances }: HorizonModsPageProps) {
  const horizonInstances = instances.filter((instance) => instance.engine === "horizon");
  const selected = horizonInstances.find((instance) => instance.id === selectedInstanceId) ?? horizonInstances[0] ?? null;
  const [mods, setMods] = useState<HorizonMod[]>([]);
  const [path, setPath] = useState("");
  const [loading, setLoading] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState("");
  const [pendingRemove, setPendingRemove] = useState<HorizonMod | null>(null);

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

  useEffect(() => { void refresh(); }, [selected?.id]);

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
        <div><p className="eyebrow"><PackageOpen size={14} /> HORIZON CONTENT</p><h1>Horizon mods</h1><p className="page-subtitle">Manage data-only <code>.hmod</code> content for the selected Horizon profile.</p></div>
        {horizonInstances.length > 0 && <div className="settings-select-wrap"><select aria-label="Horizon profile for mods" value={selected?.id ?? ""} onChange={(event) => onSelectInstance(event.target.value)}>{horizonInstances.map((instance) => <option key={instance.id} value={instance.id}>{instance.name} · 26.3</option>)}</select></div>}
      </section>
      {!selected ? (
        <section className="empty-library-card empty-library-large"><div className="empty-icon"><Boxes size={23} /></div><div><strong>Create a Horizon profile first</strong><span>Horizon mods are kept separately from Java mods and standard Minecraft instances.</span></div><button className="play-button" type="button" onClick={onOpenInstances}>Open instances</button></section>
      ) : (
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
      )}
      {pendingRemove && <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setPendingRemove(null); }}><section className="modal-card small-modal" role="alertdialog" aria-modal="true" aria-labelledby="horizon-mod-remove-title"><div className="modal-header"><div><p className="eyebrow danger-eyebrow"><CircleAlert size={14} /> REMOVE MOD</p><h2 id="horizon-mod-remove-title">Remove {pendingRemove.name ?? pendingRemove.fileName}?</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setPendingRemove(null)}>×</button></div><p className="modal-copy">This removes the selected <code>.hmod</code> file from {selected?.name}. Its world and other profile files stay in place.</p><div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setPendingRemove(null)}>Keep mod</button><button className="danger-button" type="button" onClick={() => void remove()}><Trash2 size={14} /> Remove mod</button></div></section></div>}
    </>
  );
}
