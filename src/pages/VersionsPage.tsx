import { useMemo, useState } from "react";
import { ArrowDownToLine, Check, FolderSearch, HardDrive, Layers3, Plus, RefreshCw, Search } from "lucide-react";
import type { Instance, MinecraftInstallation, RemoteMinecraftVersion } from "../types";

type VersionFilter = "release" | "snapshot" | "historical" | "installed";

interface VersionsPageProps {
  versions: RemoteMinecraftVersion[];
  loading: boolean;
  error: string;
  onRefresh: () => void;
  onInstall: (version: RemoteMinecraftVersion) => void;
  onCreateHorizon: () => void;
  installations: MinecraftInstallation[];
  instances: Instance[];
  scanningInstallations: boolean;
  onScanInstallations: () => void;
  onAddDetected: (installation: MinecraftInstallation, versionId: string) => void;
  installing: boolean;
  desktop: boolean;
}

export function VersionsPage({ versions, loading, error, onRefresh, onInstall, onCreateHorizon, installations, instances, scanningInstallations, onScanInstallations, onAddDetected, installing, desktop }: VersionsPageProps) {
  const [filter, setFilter] = useState<VersionFilter>("release");
  const [query, setQuery] = useState("");
  const matchingVersions = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    if (filter === "installed") return [];
    return versions.filter((version) => {
      const matchesFilter = filter === "historical"
        ? version.kind === "old_alpha" || version.kind === "old_beta"
        : version.kind === filter;
      const matchesSearch = !normalizedQuery
        || `${version.id} ${version.kind} ${version.releaseTime}`.toLocaleLowerCase().includes(normalizedQuery);
      return matchesFilter && matchesSearch;
    });
  }, [filter, query, versions]);
  const matchingInstallations = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return installations.flatMap((installation) => installation.versions.map((version) => ({ installation, version })))
      .filter(({ installation, version }) => !normalizedQuery || `${version.id} ${version.kind} ${installation.root}`.toLocaleLowerCase().includes(normalizedQuery));
  }, [installations, query]);

  const discoveredVersionCount = installations.reduce((total, installation) => total + installation.versions.length, 0);

  return (
    <>
      <section className="welcome-row subpage-title">
        <div>
          <p className="eyebrow">OFFICIAL GAME VERSIONS</p>
          <h1>Minecraft versions</h1>
          <p className="page-subtitle">Browse Mojang’s version list and install supported vanilla versions into separate instance folders. Very old metadata formats may not be supported yet.</p>
        </div>
        <button className="secondary-button" type="button" onClick={onRefresh} disabled={!desktop || loading}>
          {loading ? "Refreshing…" : "Refresh versions"}
        </button>
      </section>

      {!desktop && <div className="notice-banner"><ArrowDownToLine size={17} /><span>Open OpnLauncher as a desktop app to browse and install Minecraft versions.</span></div>}

      <section className="horizon-version-card">
        <span className="version-note-icon"><Layers3 size={18} /></span>
        <div><p className="eyebrow">SEPARATE RUST GAME ENGINE</p><strong>Minecraft 26.3 · Horizon</strong><small>Create an offline Horizon profile. OpnLauncher can download and prepare the official 26.3 game files for it.</small></div>
        <button className="secondary-button" type="button" disabled={!desktop || installing} onClick={onCreateHorizon}>Create Horizon profile</button>
      </section>

      <section className="version-manager-panel">
          <div className="version-manager-note">
          <span className="version-note-icon"><Layers3 size={17} /></span>
          <span><strong>Vanilla releases, snapshots, and historical versions</strong><small>Downloads come directly from Minecraft’s official metadata and file hosts. No game files are requested until you choose Install.</small></span>
        </div>
        <div className="version-toolbar">
          <div className="search-box"><Search size={15} /><input aria-label="Search Minecraft versions" placeholder="Search versions" value={query} onChange={(event) => setQuery(event.target.value)} /></div>
          <div className="version-filters" role="group" aria-label="Filter Minecraft versions">
            <button className={filter === "release" ? "selected" : ""} type="button" onClick={() => setFilter("release")}>Releases</button>
            <button className={filter === "snapshot" ? "selected" : ""} type="button" onClick={() => setFilter("snapshot")}>Snapshots</button>
            <button className={filter === "historical" ? "selected" : ""} type="button" onClick={() => setFilter("historical")}>Historical</button>
            <button className={filter === "installed" ? "selected" : ""} type="button" onClick={() => setFilter("installed")}>On this device <small>{discoveredVersionCount}</small></button>
          </div>
        </div>
        {filter === "installed" && <div className="detected-version-summary"><span><FolderSearch size={16} /><strong>{installations.length} game folder{installations.length === 1 ? "" : "s"} found</strong><small>{discoveredVersionCount} installed version{discoveredVersionCount === 1 ? "" : "s"} discovered automatically on this device.</small></span><button className="secondary-button" type="button" onClick={onScanInstallations} disabled={!desktop || scanningInstallations}><RefreshCw className={scanningInstallations ? "spin" : ""} size={14} />{scanningInstallations ? "Scanning…" : "Scan again"}</button></div>}
        {error && <div className="error-banner" role="alert">{error}</div>}
        {filter === "installed" ? installations.length > 0 ? (
          <div className="official-version-list discovered-version-list">
            {matchingInstallations.map(({ installation, version }) => {
              const alreadyAdded = instances.some((instance) => instance.engine === "minecraft" && instance.gameDir === installation.root && instance.versionId === version.id);
              return <article className="official-version-row discovered-version-row" key={`${installation.root}:${version.id}`}>
                <span className="version-row-mark"><HardDrive size={16} /></span>
                <span className="official-version-copy"><strong>{version.id}</strong><small>{version.kind.replace("_", " ")} · {installation.root}</small></span>
                <button className={alreadyAdded ? "secondary-button" : "play-button"} type="button" onClick={() => onAddDetected(installation, version.id)} disabled={!desktop || alreadyAdded}>
                  {alreadyAdded ? <><Check size={14} /> Added</> : <><Plus size={14} /> Add to library</>}
                </button>
              </article>;
            })}
            {matchingInstallations.length === 0 && <div className="version-empty-state">No installed versions match this search.</div>}
          </div>
        ) : <div className="version-empty-state"><span className="mod-empty-icon"><FolderSearch size={20} /></span><strong>No Minecraft installations found yet</strong><span>OpnLauncher scans standard launcher folders when it starts. Scan again after installing a game or choose Add instance to inspect a custom folder.</span><button className="secondary-button" type="button" onClick={onScanInstallations} disabled={!desktop || scanningInstallations}><RefreshCw size={14} /> Scan this device</button></div> : loading && versions.length === 0 ? (
          <div className="version-empty-state">Loading Mojang’s version list…</div>
        ) : matchingVersions.length > 0 ? (
          <div className="official-version-list">
            {matchingVersions.slice(0, 120).map((version) => (
              <article className="official-version-row" key={version.id}>
                <span className="version-row-mark"><Layers3 size={16} /></span>
                <span className="official-version-copy"><strong>{version.id}</strong><small>{version.kind.replace("_", " ")} · {version.releaseTime.slice(0, 10) || "Release date unavailable"}</small></span>
                  <button className="secondary-button" type="button" onClick={() => onInstall(version)} disabled={!desktop || installing}>
                  <ArrowDownToLine size={14} /> Install
                </button>
              </article>
            ))}
            {matchingVersions.length > 120 && <p className="version-list-note">Showing the first 120 matches. Search for an older version to narrow the list.</p>}
          </div>
        ) : (
          <div className="version-empty-state">{error ? "The version list could not be loaded." : "No versions match this search."}</div>
        )}
        {filter !== "installed" && versions.length > 0 && <p className="version-list-note">{matchingVersions.length} matching version{matchingVersions.length === 1 ? "" : "s"}</p>}
      </section>
    </>
  );
}
