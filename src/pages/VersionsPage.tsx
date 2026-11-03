import { useMemo, useState } from "react";
import { ArrowDownToLine, Layers3, Search } from "lucide-react";
import type { RemoteMinecraftVersion } from "../types";

type VersionFilter = "release" | "snapshot" | "historical";

interface VersionsPageProps {
  versions: RemoteMinecraftVersion[];
  loading: boolean;
  error: string;
  onRefresh: () => void;
  onInstall: (version: RemoteMinecraftVersion) => void;
  installing: boolean;
  desktop: boolean;
}

export function VersionsPage({ versions, loading, error, onRefresh, onInstall, installing, desktop }: VersionsPageProps) {
  const [filter, setFilter] = useState<VersionFilter>("release");
  const [query, setQuery] = useState("");
  const matchingVersions = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return versions.filter((version) => {
      const matchesFilter = filter === "historical"
        ? version.kind === "old_alpha" || version.kind === "old_beta"
        : version.kind === filter;
      const matchesSearch = !normalizedQuery
        || `${version.id} ${version.kind} ${version.releaseTime}`.toLocaleLowerCase().includes(normalizedQuery);
      return matchesFilter && matchesSearch;
    });
  }, [filter, query, versions]);

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
          </div>
        </div>
        {error && <div className="error-banner" role="alert">{error}</div>}
        {loading && versions.length === 0 ? (
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
        {versions.length > 0 && <p className="version-list-note">{matchingVersions.length} matching version{matchingVersions.length === 1 ? "" : "s"}</p>}
      </section>
    </>
  );
}
