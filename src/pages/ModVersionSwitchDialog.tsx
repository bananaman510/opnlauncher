import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertCircle, ArrowDownUp, Check, ChevronDown, LoaderCircle, Search, ShieldAlert, X } from "lucide-react";
import { filterAndSortModVersions, type ModReleaseFilter, type ModVersionSort } from "../modVersions";
import type { InstalledMod, ModInstallSource, ModInstanceContext, ModProject, ModProvider, ModSearchResult, ModVersion } from "../types";

interface ModVersionSwitchDialogProps {
  mod: InstalledMod;
  instanceId: string;
  context: ModInstanceContext | null;
  initialProvider: ModProvider;
  curseForgeKey: string;
  switching: boolean;
  onClose: () => void;
  onRequestCurseForgeKey: () => void;
  onSwitch: (mod: InstalledMod, source: ModInstallSource, version: ModVersion) => Promise<void>;
}

function dateLabel(value: string): string {
  const parsed = new Date(value);
  return value && !Number.isNaN(parsed.getTime()) ? parsed.toLocaleDateString() : "Date unavailable";
}

export function ModVersionSwitchDialog(props: ModVersionSwitchDialogProps) {
  const [source, setSource] = useState<ModInstallSource | null>(props.mod.source);
  const [project, setProject] = useState<ModProject | null>(null);
  const [versions, setVersions] = useState<ModVersion[]>([]);
  const [projectChoices, setProjectChoices] = useState<ModProject[]>([]);
  const [provider, setProvider] = useState<ModProvider>(props.mod.source?.provider ?? props.initialProvider);
  const [projectQuery, setProjectQuery] = useState(props.mod.modId || props.mod.name);
  const [versionQuery, setVersionQuery] = useState("");
  const [gameVersionFilter, setGameVersionFilter] = useState("");
  const [loaderFilter, setLoaderFilter] = useState("");
  const [releaseFilter, setReleaseFilter] = useState<ModReleaseFilter>("all");
  const [sort, setSort] = useState<ModVersionSort>("newest");
  const [selectedVersionId, setSelectedVersionId] = useState("");
  const [loadingSource, setLoadingSource] = useState(!props.mod.source);
  const [loadingProject, setLoadingProject] = useState(false);
  const [searchingProjects, setSearchingProjects] = useState(false);
  const [linkingProjectId, setLinkingProjectId] = useState("");
  const [error, setError] = useState("");
  const [confirmSwitch, setConfirmSwitch] = useState(false);

  useEffect(() => {
    if (props.mod.source) return;
    let cancelled = false;
    setLoadingSource(true);
    invoke<ModInstallSource | null>("resolve_installed_mod_source", { instanceId: props.instanceId, fileName: props.mod.fileName })
      .then((resolved) => {
        if (!cancelled && resolved) {
          setSource(resolved);
          setProvider(resolved.provider);
        }
      })
      .catch((reason: unknown) => { if (!cancelled) setError(String(reason)); })
      .finally(() => { if (!cancelled) setLoadingSource(false); });
    return () => { cancelled = true; };
  }, [props.instanceId, props.mod.fileName, props.mod.source]);

  useEffect(() => {
    if (!source) return;
    if (source.provider === "curseforge" && !props.curseForgeKey.trim()) {
      setError("Enter your CurseForge API key to view this project's versions.");
      return;
    }
    let cancelled = false;
    setLoadingProject(true);
    setError("");
    const apiKey = source.provider === "curseforge" ? props.curseForgeKey || null : null;
    Promise.all([
      invoke<ModProject>("get_mod_project", { provider: source.provider, projectId: source.projectId, apiKey }),
      invoke<ModVersion[]>("get_all_mod_project_versions", { provider: source.provider, projectId: source.projectId, apiKey }),
    ])
      .then(([details, available]) => {
        if (cancelled) return;
        setProject(details);
        setVersions(available);
        if (!available.length) setError("No releases were returned for this project.");
      })
      .catch((reason: unknown) => { if (!cancelled) setError(String(reason)); })
      .finally(() => { if (!cancelled) setLoadingProject(false); });
    return () => { cancelled = true; };
  }, [props.curseForgeKey, source]);

  const gameVersions = useMemo(() => [...new Set(versions.flatMap((version) => version.gameVersions))].sort((left, right) => right.localeCompare(left, undefined, { numeric: true })), [versions]);
  const modLoaders = useMemo(() => [...new Set(versions.flatMap((version) => version.loaders))].sort((left, right) => left.localeCompare(right)), [versions]);

  useEffect(() => {
    if (!versions.length) return;
    const currentGameVersion = props.context?.minecraftVersion ?? "";
    const currentLoader = props.context?.loader ?? "";
    setGameVersionFilter(currentGameVersion && gameVersions.includes(currentGameVersion) ? currentGameVersion : "");
    setLoaderFilter(currentLoader && modLoaders.includes(currentLoader) ? currentLoader : "");
    const existing = source?.versionId && versions.some((version) => version.id === source.versionId) ? source.versionId : "";
    const compatible = versions.find((version) => currentGameVersion && currentLoader
      && version.gameVersions.includes(currentGameVersion)
      && version.loaders.includes(currentLoader));
    setSelectedVersionId(existing || compatible?.id || versions[0].id);
  }, [gameVersions, modLoaders, props.context?.loader, props.context?.minecraftVersion, source?.versionId, versions]);

  const filteredVersions = useMemo(() => filterAndSortModVersions(versions, {
    query: versionQuery,
    gameVersion: gameVersionFilter,
    loader: loaderFilter,
    releaseType: releaseFilter,
    sort,
  }), [gameVersionFilter, loaderFilter, releaseFilter, sort, versionQuery, versions]);

  const selectedVersion = versions.find((version) => version.id === selectedVersionId) ?? null;
  const currentGameVersion = props.context?.minecraftVersion ?? "";
  const currentLoader = props.context?.loader ?? "";
  const selectedIsCompatible = Boolean(selectedVersion && currentGameVersion && currentLoader
    && selectedVersion.gameVersions.includes(currentGameVersion)
    && selectedVersion.loaders.includes(currentLoader));
  const selectedIsCurrent = Boolean(selectedVersion && source?.versionId === selectedVersion.id);

  async function searchForProject() {
    if (!props.context?.minecraftVersion || !props.context.loader) {
      setError("This instance needs readable Minecraft and loader metadata to search compatible projects.");
      return;
    }
    if (provider === "curseforge" && !props.curseForgeKey.trim()) {
      setError("Enter your CurseForge API key before searching that provider.");
      return;
    }
    setSearchingProjects(true);
    setError("");
    try {
      const result = await invoke<ModSearchResult>("search_mod_projects", {
        provider,
        query: projectQuery,
        gameVersion: props.context.minecraftVersion,
        loader: props.context.loader,
        sort: "relevance",
        offset: 0,
        apiKey: provider === "curseforge" ? props.curseForgeKey || null : null,
      });
      setProjectChoices(result.projects);
      if (!result.projects.length) setError("No matching project was found for this instance. Try its exact mod name or ID.");
    } catch (reason) {
      setError(String(reason));
      setProjectChoices([]);
    } finally {
      setSearchingProjects(false);
    }
  }

  async function chooseProject(candidate: ModProject) {
    setLinkingProjectId(candidate.id);
    setError("");
    try {
      const linked = await invoke<ModInstallSource>("link_installed_mod_to_project", {
        instanceId: props.instanceId,
        fileName: props.mod.fileName,
        provider: candidate.provider,
        projectId: candidate.id,
        apiKey: candidate.provider === "curseforge" ? props.curseForgeKey || null : null,
      });
      setSource(linked);
      setProvider(linked.provider);
      setProject(candidate);
      setProjectChoices([]);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLinkingProjectId("");
    }
  }

  async function confirmSelectedVersion() {
    if (!source || !selectedVersion || !selectedIsCompatible || selectedIsCurrent) return;
    setConfirmSwitch(false);
    try {
      await props.onSwitch(props.mod, source, selectedVersion);
      props.onClose();
    } catch (reason) {
      setError(String(reason));
    }
  }

  return (
    <div className="modal-backdrop confirmation-backdrop mod-switch-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !props.switching) props.onClose(); }}>
      <section className="modal-card mod-switch-modal" role="dialog" aria-modal="true" aria-labelledby="mod-switch-title">
        <div className="modal-header"><div><p className="eyebrow"><ArrowDownUp size={14} /> MOD VERSION MANAGER</p><h2 id="mod-switch-title">Switch {props.mod.name} version</h2><small className="mod-switch-current">Current file: {props.mod.fileName}</small></div><button className="icon-button" type="button" aria-label="Close version manager" onClick={props.onClose} disabled={props.switching}><X size={18} /></button></div>

        {loadingSource && <div className="mod-switch-loading"><LoaderCircle className="spin" size={19} /> Checking this mod's provider record…</div>}
        {!loadingSource && !source && <section className="mod-project-linker">
          <div><strong>Find this mod's project</strong><small>This file predates project tracking. Search by its name or mod ID, then choose the matching project.</small></div>
          <div className="mod-switch-provider" role="group" aria-label="Provider for project lookup">
            <button type="button" className={provider === "modrinth" ? "selected" : ""} onClick={() => { setProvider("modrinth"); setProjectChoices([]); }}>Modrinth</button>
            <button type="button" className={provider === "curseforge" ? "selected" : ""} onClick={() => { setProvider("curseforge"); setProjectChoices([]); }}>CurseForge</button>
          </div>
          <div className="mod-switch-search"><input className="field-control" aria-label="Search for this mod project" value={projectQuery} onChange={(event) => setProjectQuery(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") void searchForProject(); }} /><button className="secondary-button" type="button" onClick={() => void searchForProject()} disabled={searchingProjects || !projectQuery.trim()}>{searchingProjects ? <LoaderCircle className="spin" size={15} /> : <Search size={15} />} Search</button></div>
          {provider === "curseforge" && !props.curseForgeKey.trim() && <div className="mod-version-warning"><AlertCircle size={15} /> CurseForge requires your session API key.<button type="button" className="text-link" onClick={props.onRequestCurseForgeKey}>Enter key</button></div>}
          {projectChoices.length > 0 && <div className="mod-project-choice-list">{projectChoices.map((candidate) => <button type="button" key={candidate.id} onClick={() => void chooseProject(candidate)} disabled={Boolean(linkingProjectId)}><span className="mod-project-icon">{candidate.iconUrl ? <img src={candidate.iconUrl} alt="" /> : <Search size={16} />}</span><span><strong>{candidate.title}</strong><small>by {candidate.author} · {candidate.provider === "modrinth" ? "Modrinth" : "CurseForge"}</small></span>{linkingProjectId === candidate.id ? <LoaderCircle className="spin" size={15} /> : <Check size={15} />}</button>)}</div>}
        </section>}

        {source && source.provider === "curseforge" && !props.curseForgeKey.trim() && <div className="mod-version-warning"><AlertCircle size={15} /> Enter your CurseForge key to view releases.<button type="button" className="text-link" onClick={props.onRequestCurseForgeKey}>Enter key</button></div>}
        {source && (!loadingProject || project) && project && <div className="mod-switch-project"><span className="mod-project-icon">{project.iconUrl ? <img src={project.iconUrl} alt="" /> : <ArrowDownUp size={17} />}</span><span><strong>{project.title}</strong><small>Project releases · {versions.length.toLocaleString()} loaded</small></span><span className={`mod-provider-pill provider-pill-${project.provider}`}>{project.provider === "modrinth" ? "MODRINTH" : "CURSEFORGE"}</span></div>}
        {source && loadingProject && <div className="mod-switch-loading"><LoaderCircle className="spin" size={19} /> Loading project releases…</div>}
        {error && <p className="instance-setup-error mod-switch-error" role="alert"><AlertCircle size={15} />{error}</p>}

        {source && versions.length > 0 && <>
          <div className="mod-version-filter-grid">
            <label><span>Search releases</span><input className="field-control" value={versionQuery} onChange={(event) => setVersionQuery(event.target.value)} placeholder="Name or version" /></label>
            <label><span>Minecraft version</span><span className="select-wrap"><select className="field-control" value={gameVersionFilter} onChange={(event) => setGameVersionFilter(event.target.value)}><option value="">All Minecraft versions</option>{gameVersions.map((version) => <option key={version} value={version}>{version}</option>)}</select><ChevronDown size={14} /></span></label>
            <label><span>Mod loader</span><span className="select-wrap"><select className="field-control" value={loaderFilter} onChange={(event) => setLoaderFilter(event.target.value)}><option value="">All loaders</option>{modLoaders.map((loader) => <option key={loader} value={loader}>{loader[0]?.toUpperCase()}{loader.slice(1)}</option>)}</select><ChevronDown size={14} /></span></label>
            <label><span>Release type</span><span className="select-wrap"><select className="field-control" value={releaseFilter} onChange={(event) => setReleaseFilter(event.target.value as ModReleaseFilter)}><option value="all">All releases</option><option value="release">Release</option><option value="beta">Beta</option><option value="alpha">Alpha</option></select><ChevronDown size={14} /></span></label>
            <label><span>Sort</span><span className="select-wrap"><select className="field-control" value={sort} onChange={(event) => setSort(event.target.value as ModVersionSort)}><option value="newest">Newest first</option><option value="oldest">Oldest first</option><option value="version-az">Version A–Z</option><option value="version-za">Version Z–A</option></select><ChevronDown size={14} /></span></label>
          </div>
          <div className="mod-switch-target"><span>Switching for</span><strong>{currentGameVersion || "Minecraft version unavailable"} · {currentLoader || "loader unavailable"}</strong><small>Versions that do not match this instance can be browsed but cannot be installed here.</small></div>
          <div className="mod-switch-version-list" role="listbox" aria-label="Available mod versions">
            {filteredVersions.map((version) => {
              const compatible = Boolean(currentGameVersion && currentLoader && version.gameVersions.includes(currentGameVersion) && version.loaders.includes(currentLoader));
              return <button type="button" role="option" aria-selected={selectedVersionId === version.id} key={version.id} className={`mod-switch-version ${selectedVersionId === version.id ? "selected" : ""} ${compatible ? "" : "incompatible"}`} onClick={() => setSelectedVersionId(version.id)}>
                <span className="mod-version-check">{selectedVersionId === version.id ? <Check size={13} /> : <span />}</span><span className="mod-version-copy"><strong>{version.name}</strong><small>{version.versionNumber} · {version.releaseType} · {dateLabel(version.datePublished)}</small><small>Minecraft: {version.gameVersions.join(", ") || "not specified"} · Loaders: {version.loaders.join(", ") || "not specified"}</small></span><span className={`mod-switch-compat ${compatible ? "compatible" : "incompatible"}`}>{compatible ? "Compatible" : "Different setup"}</span>
              </button>;
            })}
            {!filteredVersions.length && <div className="mod-version-empty"><Search size={16} />No releases match those filters.</div>}
          </div>
        </>}

        <div className="mod-switch-footer"><small><ShieldAlert size={13} /> Switching replaces this mod file after the new download passes integrity checks. Required dependencies are added; other mods, saves, and settings are kept.</small><button className="play-button" type="button" disabled={!source || !selectedVersion || !selectedIsCompatible || selectedIsCurrent || props.switching} onClick={() => setConfirmSwitch(true)}>{props.switching ? <LoaderCircle className="spin" size={15} /> : <ArrowDownUp size={15} />}{props.switching ? "Switching…" : selectedIsCurrent ? "Current version" : "Switch version"}</button></div>
      </section>

      {confirmSwitch && selectedVersion && <div className="modal-backdrop confirmation-backdrop" role="presentation"><section className="modal-card small-modal" role="alertdialog" aria-modal="true" aria-labelledby="mod-switch-confirm-title"><div className="modal-header"><div><p className="eyebrow"><ArrowDownUp size={14} /> REPLACE MOD FILE</p><h2 id="mod-switch-confirm-title">Switch to {selectedVersion.versionNumber}?</h2></div><button className="icon-button" type="button" aria-label="Close confirmation" onClick={() => setConfirmSwitch(false)}><X size={18} /></button></div><p className="modal-copy">OpnLauncher downloads and verifies the selected version before replacing <code>{props.mod.fileName}</code>. If the new download fails, the current file stays in place.</p><div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setConfirmSwitch(false)}>Cancel</button><button className="play-button" type="button" onClick={() => void confirmSelectedVersion()}><ArrowDownUp size={15} /> Switch version</button></div></section></div>}
    </div>
  );
}
