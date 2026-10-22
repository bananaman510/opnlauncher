import { useEffect, useMemo, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  ArrowDownToLine,
  Boxes,
  Check,
  ChevronDown,
  CircleAlert,
  Clock3,
  Command,
  Cpu,
  FolderOpen,
  HardDrive,
  House,
  Layers3,
  LoaderCircle,
  MonitorCog,
  PackageOpen,
  Play,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
  TerminalSquare,
  UsersRound,
  X,
} from "lucide-react";
import { currentDateLabel, formatRelativeTime, javaLabel } from "./format";
import type {
  Instance,
  JavaRuntime,
  LaunchExitEvent,
  LaunchLogEvent,
  LaunchStarted,
  MinecraftInstallation,
  OfflineProfile,
} from "./types";

const navigation = [
  { id: "home", label: "Home", icon: House },
  { id: "instances", label: "Instances", icon: Boxes },
  { id: "versions", label: "Minecraft versions", icon: Layers3 },
  { id: "mods", label: "Mods", icon: PackageOpen },
  { id: "modpacks", label: "Modpacks", icon: FolderOpen },
  { id: "accounts", label: "Accounts", icon: UsersRound },
  { id: "downloads", label: "Downloads", icon: ArrowDownToLine },
  { id: "settings", label: "Settings", icon: Settings2 },
] as const;

const isDevelopmentProfileBuild = import.meta.env.DEV;

function App() {
  const desktop = isTauri();
  const [activePage, setActivePage] = useState<string>("home");
  const [instances, setInstances] = useState<Instance[]>([]);
  const [installations, setInstallations] = useState<MinecraftInstallation[]>([]);
  const [runtimes, setRuntimes] = useState<JavaRuntime[]>([]);
  const [profiles, setProfiles] = useState<OfflineProfile[]>([]);
  const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(null);
  const [selectedProfileId, setSelectedProfileId] = useState("");
  const [selectedJavaPath, setSelectedJavaPath] = useState("");
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [showProfileForm, setShowProfileForm] = useState(false);
  const [instanceName, setInstanceName] = useState("");
  const [profileName, setProfileName] = useState("");
  const [installPath, setInstallPath] = useState("");
  const [inspectedInstall, setInspectedInstall] = useState<MinecraftInstallation | null>(null);
  const [versionChoice, setVersionChoice] = useState("");
  const [memoryChoice, setMemoryChoice] = useState(2048);
  const [instanceSearch, setInstanceSearch] = useState("");
  const [isBusy, setIsBusy] = useState(false);
  const [busyMessage, setBusyMessage] = useState("");
  const [error, setError] = useState("");
  const [logs, setLogs] = useState<string[]>([]);
  const [launchState, setLaunchState] = useState<"idle" | "running" | "exited">("idle");
  const [logPath, setLogPath] = useState("");
  const selectedInstanceIdRef = useRef(selectedInstanceId);
  selectedInstanceIdRef.current = selectedInstanceId;

  const selectedInstance = instances.find((instance) => instance.id === selectedInstanceId) ?? null;
  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId) ?? null;
  const selectedJava = runtimes.find((runtime) => runtime.path === selectedJavaPath) ?? null;
  const latestLogLines = useMemo(() => logs.slice(-100), [logs]);
  const visibleInstances = useMemo(() => {
    const query = instanceSearch.trim().toLocaleLowerCase();
    return query
      ? instances.filter((instance) => `${instance.name} ${instance.versionId ?? ""} ${instance.gameDir}`.toLocaleLowerCase().includes(query))
      : instances;
  }, [instances, instanceSearch]);

  useEffect(() => {
    if (!desktop) return;
    let cancelled = false;
    let unlistenLog: UnlistenFn | undefined;
    let unlistenExit: UnlistenFn | undefined;

    const load = async () => {
      setIsBusy(true);
      setBusyMessage("Checking this device");
      try {
        const [savedInstances, foundInstallations, foundRuntimes] = await Promise.all([
          invoke<Instance[]>("list_instances"),
          invoke<MinecraftInstallation[]>("scan_installations"),
          invoke<JavaRuntime[]>("list_java_runtimes"),
        ]);
        if (cancelled) return;
        setInstances(savedInstances);
        setInstallations(foundInstallations);
        setRuntimes(foundRuntimes);
        if (savedInstances.length > 0) {
          setSelectedInstanceId((current) => current ?? savedInstances[0].id);
          setMemoryChoice(savedInstances[0].memoryMb);
        }
        if (foundRuntimes.length > 0) setSelectedJavaPath(foundRuntimes[0].path);
        if (isDevelopmentProfileBuild) {
          const savedProfiles = await invoke<OfflineProfile[]>("list_offline_profiles");
          if (!cancelled) {
            setProfiles(savedProfiles);
            setSelectedProfileId(savedProfiles[0]?.id ?? "");
          }
        }
      } catch (reason) {
        if (!cancelled) setError(String(reason));
      } finally {
        if (!cancelled) setIsBusy(false);
      }
    };

    void load();
    void listen<LaunchLogEvent>("launch-log", (event) => {
      if (event.payload.instanceId === selectedInstanceIdRef.current) {
        setLogs((current) => [...current, `[${event.payload.stream}] ${event.payload.text}`]);
      }
    }).then((dispose) => {
      unlistenLog = dispose;
    });
    void listen<LaunchExitEvent>("launch-exit", (event) => {
      if (event.payload.instanceId === selectedInstanceIdRef.current) {
        setLaunchState("exited");
        setLogs((current) => [
          ...current,
          event.payload.success
            ? `Minecraft closed successfully (exit ${event.payload.code ?? "unknown"}).`
            : `Minecraft exited with code ${event.payload.code ?? "unknown"}.`,
        ]);
        void refreshInstances();
      }
    }).then((dispose) => {
      unlistenExit = dispose;
    });
    return () => {
      cancelled = true;
      unlistenLog?.();
      unlistenExit?.();
    };
    // The selected instance ref keeps event subscriptions stable while selection changes.
  }, [desktop]);

  useEffect(() => {
    if (selectedInstance) {
      setMemoryChoice(selectedInstance.memoryMb);
      setSelectedJavaPath(selectedInstance.javaPath ?? runtimes[0]?.path ?? "");
    }
  }, [selectedInstanceId, instances, runtimes]);

  async function refreshInstances() {
    const savedInstances = await invoke<Instance[]>("list_instances");
    setInstances(savedInstances);
  }

  async function refreshInstallations() {
    const found = await invoke<MinecraftInstallation[]>("scan_installations");
    setInstallations(found);
  }

  async function addDetectedInstallation(installation: MinecraftInstallation, versionId: string) {
    const suggested = `${versionId} · ${installation.root.split(/[\/]/).filter(Boolean).at(-1) ?? "Minecraft"}`;
    const name = window.prompt("Name this instance", suggested);
    if (!name?.trim()) return;
    setError("");
    setIsBusy(true);
    setBusyMessage("Adding local installation");
    try {
      const created = await invoke<Instance>("register_installation", {
        name,
        root: installation.root,
        versionId,
      });
      setInstances((current) => [created, ...current]);
      setSelectedInstanceId(created.id);
      setShowAddDialog(false);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsBusy(false);
    }
  }

  async function inspectCustomInstallation() {
    setError("");
    setIsBusy(true);
    try {
      const inspected = await invoke<MinecraftInstallation>("inspect_installation", { path: installPath });
      setInspectedInstall(inspected);
      setVersionChoice(inspected.versions[0]?.id ?? "");
    } catch (reason) {
      setInspectedInstall(null);
      setError(String(reason));
    } finally {
      setIsBusy(false);
    }
  }

  async function addInspectedInstallation() {
    if (!inspectedInstall || !versionChoice) return;
    await addDetectedInstallation(inspectedInstall, versionChoice);
  }

  async function createBlankInstance(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!instanceName.trim()) return;
    setError("");
    setIsBusy(true);
    setBusyMessage("Creating instance");
    try {
      const created = await invoke<Instance>("create_instance", { name: instanceName });
      setInstances((current) => [created, ...current]);
      setSelectedInstanceId(created.id);
      setInstanceName("");
      setShowAddDialog(false);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsBusy(false);
    }
  }

  async function createProfile(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!profileName.trim()) return;
    setError("");
    try {
      const created = await invoke<OfflineProfile>("create_offline_profile", { name: profileName });
      setProfiles((current) => [...current, created]);
      setSelectedProfileId(created.id);
      setProfileName("");
      setShowProfileForm(false);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function saveInstanceSettings() {
    if (!selectedInstance) return;
    setError("");
    try {
      await invoke("set_instance_memory", {
        instanceId: selectedInstance.id,
        memoryMb: memoryChoice,
      });
      await invoke("set_instance_java", {
        instanceId: selectedInstance.id,
        javaPath: selectedJavaPath || null,
      });
      await refreshInstances();
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function renameSelectedInstance() {
    if (!selectedInstance) return;
    const name = window.prompt("Rename this instance", selectedInstance.name);
    if (!name?.trim() || name.trim() === selectedInstance.name) return;
    setError("");
    try {
      await invoke("rename_instance", { instanceId: selectedInstance.id, name: name.trim() });
      await refreshInstances();
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function forgetSelectedInstance() {
    if (!selectedInstance) return;
    const confirmed = window.confirm(
      `Remove “${selectedInstance.name}” from OpnLauncher? The Minecraft directory and all worlds will be kept on disk.`,
    );
    if (!confirmed) return;
    setError("");
    try {
      await invoke("forget_instance", { instanceId: selectedInstance.id, confirmed });
      const remaining = instances.filter((instance) => instance.id !== selectedInstance.id);
      setInstances(remaining);
      setSelectedInstanceId(remaining[0]?.id ?? null);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function launchSelected() {
    if (!selectedInstance || !selectedProfile) return;
    setError("");
    setLogs([]);
    setLaunchState("running");
    setBusyMessage("Starting Minecraft");
    try {
      const started = await invoke<LaunchStarted>("launch_instance", {
        instanceId: selectedInstance.id,
        profileId: selectedProfile.id,
      });
      setLogPath(started.logPath);
      setLogs((current) => [`Process ${started.pid} started. Local log: ${started.logPath}`, ...current]);
    } catch (reason) {
      setLaunchState("idle");
      setError(String(reason));
    }
  }

  const activeNavigation = navigation.find((item) => item.id === activePage) ?? navigation[0];

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark"><Command size={19} strokeWidth={2.2} /></div>
          <div className="brand-copy"><strong>OpnLauncher</strong><span>LOCAL · OPEN · YOURS</span></div>
        </div>

        <div className="nav-label">WORKSPACE</div>
        <nav className="main-nav" aria-label="Main navigation">
          {navigation.map(({ id, label, icon: Icon }) => (
            <button
              className={`nav-item ${activePage === id ? "is-active" : ""}`}
              key={id}
              onClick={() => setActivePage(id)}
              type="button"
            >
              <Icon size={17} strokeWidth={1.9} />
              <span>{label}</span>
              {id === "downloads" && <span className="nav-dot" aria-label="No active downloads" />}
            </button>
          ))}
        </nav>

        <div className="sidebar-bottom">
          <div className="privacy-card">
            <div className="privacy-icon"><ShieldCheck size={16} /></div>
            <div><strong>Private by default</strong><span>Game files stay on this device.</span></div>
          </div>
          <div className="sidebar-version"><span className="status-pip" /> Version 0.1.0 <span>DEV</span></div>
        </div>
      </aside>

      <main className="main-content">
        <header className="topbar">
          <div className="breadcrumbs"><span>Workspace</span><span className="crumb-divider">/</span><strong>{activeNavigation.label}</strong></div>
          <div className="topbar-actions">
            <div className="offline-pill"><span /> Offline ready</div>
            <button className="icon-button" type="button" aria-label="Settings" onClick={() => setActivePage("settings")}><MonitorCog size={17} /></button>
            <div className="avatar">OP</div>
          </div>
        </header>

        <div className="page-scroll">
          {activePage === "home" ? (
            <>
              <section className="welcome-row">
                <div>
                  <p className="eyebrow">{currentDateLabel()} <span className="eyebrow-line" /></p>
                  <h1>Ready when you are.</h1>
                  <p className="page-subtitle">Your Minecraft, your machine. Everything stays local.</p>
                </div>
                <div className="device-chip"><span className="device-pip" /> LOCAL DEVICE <span className="device-divider" /> LINUX</div>
              </section>

              {!desktop && (
                <div className="notice-banner"><CircleAlert size={18} /><span>Open this page in the OpnLauncher desktop app to connect local Minecraft files and Java.</span></div>
              )}

              {error && <div className="error-banner" role="alert"><CircleAlert size={17} /><span>{error}</span><button type="button" onClick={() => setError("")} aria-label="Dismiss error"><X size={16} /></button></div>}

              <section className="launch-card">
                <div className="launch-glow" />
                <div className="launch-card-top">
                  <div className="selected-instance-label"><span className="tiny-square" /> SELECTED INSTANCE</div>
                  <button className="more-link" type="button" onClick={() => setActivePage("instances")}>Manage instances <ChevronDown size={14} /></button>
                </div>
                {selectedInstance ? (
                  <div className="launch-content">
                    <div className="game-emblem"><Command size={28} strokeWidth={1.7} /><span /></div>
                    <div className="instance-summary">
                      <div className="instance-heading"><h2>{selectedInstance.name}</h2><span className="release-tag">{selectedInstance.versionId ?? "NO VERSION"}</span></div>
                      <div className="instance-details">
                        <span><Layers3 size={14} /> Vanilla</span>
                        <i />
                        <span><Cpu size={14} /> {selectedJava ? javaLabel(selectedJava.majorVersion) : "Java not found"}</span>
                        <i />
                        <span><Clock3 size={14} /> {formatRelativeTime(selectedInstance.lastPlayedAt)}</span>
                      </div>
                      <div className="launch-controls">
                        <button
                          className="play-button"
                          type="button"
                          disabled={!desktop || !selectedInstance.versionId || !selectedProfile || !selectedJava || launchState === "running" || isBusy}
                          onClick={() => void launchSelected()}
                        >
                          {launchState === "running" ? <LoaderCircle className="spin" size={17} /> : <Play size={16} fill="currentColor" />}
                          <span>{launchState === "running" ? "Launching" : "Play"}</span>
                        </button>
                        <button className="secondary-button profile-select" type="button" onClick={() => setShowProfileForm(true)} disabled={!isDevelopmentProfileBuild} title={isDevelopmentProfileBuild ? "Choose a local test profile" : "Microsoft sign-in is planned for a later phase"}>
                          <span className="profile-avatar">{selectedProfile?.name.slice(0, 1).toUpperCase() ?? "?"}</span>
                          <span>{selectedProfile?.name ?? (isDevelopmentProfileBuild ? "Select profile" : "Account support planned")}</span>
                          <ChevronDown size={14} />
                        </button>
                        <button className="secondary-button compact-button" type="button" onClick={() => setActivePage("instances")} aria-label="Edit instance"><Settings2 size={16} /></button>
                      </div>
                    </div>
                  </div>
                ) : (
                  <div className="empty-launch">
                    <div className="empty-orbit"><Command size={24} /></div>
                    <div><h2>Nothing selected yet</h2><p>Add a local Minecraft installation to get started.</p></div>
                    <button className="play-button" type="button" onClick={() => setShowAddDialog(true)}><Plus size={17} /><span>Add installation</span></button>
                  </div>
                )}
                {selectedInstance && (
                  <div className="launch-card-bottom">
                    <span><HardDrive size={14} /> {selectedInstance.gameDir}</span>
                    <button type="button" onClick={() => setActivePage("instances")}>Instance settings <ChevronDown size={14} /></button>
                  </div>
                )}
              </section>

              {isDevelopmentProfileBuild && (
                <section className="dev-profile-note">
                  <div className="note-icon"><UsersRound size={17} /></div>
                  <div><strong>Development profile</strong><span>This local test profile is unverified and cannot authenticate to online servers.</span></div>
                  <button type="button" onClick={() => setShowProfileForm(true)}>{profiles.length ? "Manage" : "Create profile"}</button>
                </section>
              )}

              <div className="section-heading-row">
                <div><p className="eyebrow">YOUR LIBRARY</p><h2>Instances <span className="count-pill">{instances.length}</span></h2></div>
                <button className="text-action" type="button" onClick={() => setActivePage("instances")}>View all <span>→</span></button>
              </div>
              {instances.length > 0 ? (
                <div className="instance-grid">
                  {instances.slice(0, 3).map((instance, index) => (
                    <button
                      className={`instance-card ${selectedInstanceId === instance.id ? "selected" : ""}`}
                      key={instance.id}
                      type="button"
                      onClick={() => setSelectedInstanceId(instance.id)}
                    >
                      <div className={`instance-art art-${index % 3}`}><Command size={20} /><span className="art-orb" /></div>
                      <div className="instance-card-info"><strong>{instance.name}</strong><span>{instance.versionId ?? "No version installed"}</span></div>
                      <div className="instance-card-time">{formatRelativeTime(instance.lastPlayedAt)}</div>
                    </button>
                  ))}
                  <button className="new-instance-card" type="button" onClick={() => setShowAddDialog(true)}><span><Plus size={18} /></span><strong>Add an instance</strong><small>Use an existing game installation</small></button>
                </div>
              ) : (
                <div className="empty-library">
                  <div className="empty-library-icon"><Boxes size={20} /></div>
                  <div><strong>Your library is ready for its first instance</strong><span>We’ll use a Minecraft installation already on this device.</span></div>
                  <button type="button" onClick={() => setShowAddDialog(true)}>Add installation <span>→</span></button>
                </div>
              )}

              {launchState !== "idle" && (
                <section className="console-panel">
                  <div className="console-heading"><div><TerminalSquare size={16} /><strong>Local game output</strong><span className={`console-status ${launchState}`}>{launchState === "running" ? "RUNNING" : "EXITED"}</span></div><span>{logPath}</span></div>
                  <div className="console-body" role="log" aria-live="polite">{latestLogLines.map((line, index) => <code key={`${index}-${line}`}>{line}</code>)}</div>
                </section>
              )}

              <section className="bottom-cards">
                <div className="info-card">
                  <div className="info-card-icon green"><ShieldCheck size={17} /></div>
                  <div><strong>Local by design</strong><span>No analytics, telemetry, or launcher account.</span></div>
                  <span className="info-card-arrow">↗</span>
                </div>
                <div className="info-card">
                  <div className="info-card-icon violet"><ArrowDownToLine size={17} /></div>
                  <div><strong>Nothing downloading</strong><span>Installations only read files already on your device.</span></div>
                  <span className="info-card-arrow">↗</span>
                </div>
              </section>
            </>
          ) : activePage === "instances" ? (
            <>
              <section className="welcome-row subpage-title"><div><p className="eyebrow">LOCAL LIBRARY</p><h1>Instances</h1><p className="page-subtitle">Keep your game profiles organized on this device.</p></div><button className="play-button" onClick={() => setShowAddDialog(true)} type="button"><Plus size={16} /> Add instance</button></section>
              {error && <div className="error-banner" role="alert"><CircleAlert size={17} /><span>{error}</span><button type="button" onClick={() => setError("")} aria-label="Dismiss error"><X size={16} /></button></div>}
              <div className="instance-list-toolbar"><div className="search-box"><Search size={16} /><input aria-label="Search instances" placeholder="Search your instances" value={instanceSearch} onChange={(event) => setInstanceSearch(event.target.value)} /></div><button className="secondary-button" type="button" onClick={() => void refreshInstallations()}>Rescan local files</button></div>
              {instances.length === 0 ? <div className="empty-library large-empty"><div className="empty-library-icon"><Boxes size={20} /></div><div><strong>No instances yet</strong><span>Connect an existing Minecraft installation to create your first instance.</span></div><button type="button" onClick={() => setShowAddDialog(true)}>Add installation <span>→</span></button></div> : visibleInstances.length === 0 ? <div className="empty-library large-empty"><div className="empty-library-icon"><Search size={20} /></div><div><strong>No matching instances</strong><span>Try a different name, version, or directory path.</span></div></div> : (
                <div className="management-layout">
                  <div className="management-list">{visibleInstances.map((instance) => <button className={`management-row ${selectedInstanceId === instance.id ? "selected" : ""}`} key={instance.id} onClick={() => setSelectedInstanceId(instance.id)} type="button"><span className="row-mark"><Command size={16} /></span><span className="row-copy"><strong>{instance.name}</strong><small>{instance.versionId ?? "No version installed"} · {formatRelativeTime(instance.lastPlayedAt)}</small></span><span className="row-state">LOCAL</span></button>)}</div>
                  {selectedInstance && <section className="settings-panel"><div className="settings-panel-header"><div className="game-emblem small-emblem"><Command size={22} /></div><div><strong>{selectedInstance.name}</strong><span>{selectedInstance.versionId ?? "No Minecraft version selected"}</span></div></div><label className="field-label" htmlFor="java-path">Java runtime</label><select id="java-path" className="field-control" value={selectedJavaPath} onChange={(event) => setSelectedJavaPath(event.target.value)}><option value="">Automatic detection</option>{runtimes.map((runtime) => <option key={runtime.path} value={runtime.path}>{javaLabel(runtime.majorVersion)} · {runtime.path}</option>)}</select><div className="field-row"><label className="field-label" htmlFor="memory">Memory allocation</label><span className="field-value">{memoryChoice} MB</span></div><input id="memory" className="memory-slider" type="range" min="1024" max="16384" step="512" value={memoryChoice} onChange={(event) => setMemoryChoice(Number(event.target.value))} /><div className="slider-scale"><span>1 GB</span><span>16 GB</span></div><div className="detail-path"><HardDrive size={14} /><span>{selectedInstance.gameDir}</span></div><button className="save-button" type="button" onClick={() => void saveInstanceSettings()}><Check size={15} /> Save instance settings</button><div className="instance-actions"><button className="secondary-button" type="button" onClick={() => void renameSelectedInstance()}>Rename</button><button className="forget-button" type="button" onClick={() => void forgetSelectedInstance()}>Remove from launcher</button></div><p className="preserve-note">Removing an instance keeps its game files, settings, and worlds on disk.</p></section>}
                </div>
              )}
              <form className="quick-create" onSubmit={(event) => void createBlankInstance(event)}><div><strong>Create an empty instance</strong><span>Creates a new local game directory. No game files are downloaded.</span></div><input value={instanceName} onChange={(event) => setInstanceName(event.target.value)} placeholder="Instance name" aria-label="New instance name" maxLength={48} /><button type="submit" className="secondary-button" disabled={isBusy}>Create</button></form>
            </>
          ) : activePage === "settings" ? (
            <SettingsPage runtimes={runtimes} installations={installations} refreshInstallations={() => void refreshInstallations()} />
          ) : (
            <section className="placeholder-page"><div className="placeholder-icon"><activeNavigation.icon size={22} /></div><p className="eyebrow">COMING IN A LATER PHASE</p><h1>{activeNavigation.label}</h1><p>This section will be added incrementally. OpnLauncher v0.1.0 focuses on local instances, Java discovery, and launching existing game files.</p><button className="secondary-button" type="button" onClick={() => setActivePage("home")}>Back to Home</button></section>
          )}

          <footer className="page-footer"><span>OpnLauncher is an independent community project and is not affiliated with Mojang or Microsoft.</span><span>0.1.0 · LOCAL DEVELOPMENT BUILD</span></footer>
        </div>
      </main>

      {isBusy && busyMessage && <div className="busy-toast"><LoaderCircle className="spin" size={16} /> {busyMessage}</div>}

      {showAddDialog && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowAddDialog(false); }}>
          <section className="modal-card" role="dialog" aria-modal="true" aria-labelledby="add-modal-title">
            <div className="modal-header"><div><p className="eyebrow">LOCAL FILES ONLY</p><h2 id="add-modal-title">Add an installation</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setShowAddDialog(false)}><X size={18} /></button></div>
            <p className="modal-copy">OpnLauncher checks standard Minecraft directories and reads their installed version metadata. It won’t download or modify game files.</p>
            {installations.length > 0 ? <div className="detected-list">{installations.map((installation) => <div className="detected-install" key={installation.root}><div className="detected-root"><HardDrive size={15} /><span>{installation.root}</span></div><div className="detected-version-list">{installation.versions.slice(0, 6).map((version) => <button className="version-option" key={version.id} type="button" onClick={() => void addDetectedInstallation(installation, version.id)}><span><strong>{version.id}</strong><small>{version.kind}</small></span><Plus size={15} /></button>)}</div></div>)}</div> : <div className="no-detected"><FolderOpen size={18} /><span>No standard installation found yet. Enter its game directory below.</span></div>}
            <div className="custom-path-row"><input className="field-control" value={installPath} onChange={(event) => setInstallPath(event.target.value)} placeholder="Path to .minecraft" aria-label="Custom Minecraft installation path" /><button className="secondary-button" type="button" onClick={() => void inspectCustomInstallation()} disabled={!installPath.trim() || isBusy}>Inspect</button></div>
            {inspectedInstall && <div className="inspected-version"><label className="field-label" htmlFor="version-choice">Installed version</label><select id="version-choice" className="field-control" value={versionChoice} onChange={(event) => setVersionChoice(event.target.value)}>{inspectedInstall.versions.map((version) => <option value={version.id} key={version.id}>{version.id} · {version.kind}</option>)}</select><button className="play-button" type="button" onClick={() => void addInspectedInstallation()}><Plus size={15} /> Add selected version</button></div>}
            <details className="empty-instance-details"><summary>Create an empty instance</summary><form onSubmit={(event) => void createBlankInstance(event)}><input className="field-control" value={instanceName} onChange={(event) => setInstanceName(event.target.value)} placeholder="Instance name" maxLength={48} /><button className="secondary-button" type="submit" disabled={isBusy}>Create</button></form></details>
          </section>
        </div>
      )}

      {showProfileForm && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowProfileForm(false); }}>
          <section className="modal-card profile-modal" role="dialog" aria-modal="true" aria-labelledby="profile-modal-title">
            <div className="modal-header"><div><p className="eyebrow">DEVELOPMENT ONLY</p><h2 id="profile-modal-title">Local test profile</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setShowProfileForm(false)}><X size={18} /></button></div>
            <div className="profile-warning"><CircleAlert size={17} /><span>Unverified profile. It is for local single-player development only and does not provide Microsoft authentication or online-server access.</span></div>
            {profiles.length > 0 && <div className="saved-profiles">{profiles.map((profile) => <button className={`saved-profile ${selectedProfileId === profile.id ? "selected" : ""}`} key={profile.id} type="button" onClick={() => { setSelectedProfileId(profile.id); setShowProfileForm(false); }}><span className="profile-avatar">{profile.name.slice(0, 1).toUpperCase()}</span><span><strong>{profile.name}</strong><small>UNVERIFIED · LOCAL</small></span>{selectedProfileId === profile.id && <Check size={16} />}</button>)}</div>}
            <form className="profile-create-form" onSubmit={(event) => void createProfile(event)}><label className="field-label" htmlFor="profile-name">Test username</label><input id="profile-name" className="field-control" value={profileName} onChange={(event) => setProfileName(event.target.value)} placeholder="LocalPlayer" maxLength={16} pattern="[A-Za-z0-9_]{1,16}" required /><small>Use 1–16 letters, numbers, or underscores.</small><button className="play-button" type="submit"><Plus size={15} /> Create local profile</button></form>
          </section>
        </div>
      )}
    </div>
  );
}

function SettingsPage({
  runtimes,
  installations,
  refreshInstallations,
}: {
  runtimes: JavaRuntime[];
  installations: MinecraftInstallation[];
  refreshInstallations: () => void;
}) {
  return (
    <>
      <section className="welcome-row subpage-title"><div><p className="eyebrow">PREFERENCES</p><h1>Settings</h1><p className="page-subtitle">Local app configuration and detected runtime details.</p></div></section>
      <div className="settings-cards">
        <section className="settings-panel wide-panel"><div className="settings-panel-header"><div className="info-card-icon green"><ShieldCheck size={17} /></div><div><strong>Privacy</strong><span>OpnLauncher has no telemetry, ads, or launcher account.</span></div></div><p className="settings-explainer">When you use a feature that needs network access, requests go directly to its documented provider. This first build only inspects local files and Java runtimes.</p></section>
        <section className="settings-panel wide-panel"><div className="settings-panel-header"><div className="info-card-icon violet"><Cpu size={17} /></div><div><strong>Java runtimes</strong><span>{runtimes.length} detected on this device</span></div></div>{runtimes.length ? runtimes.map((runtime) => <div className="runtime-row" key={runtime.path}><span className="runtime-mark"><Cpu size={14} /></span><div><strong>{javaLabel(runtime.majorVersion)} · {runtime.vendor ?? "Java"}</strong><small>{runtime.path}</small></div><span className="runtime-arch">{runtime.architecture ?? "Unknown arch"}</span></div>) : <p className="settings-explainer">No Java runtime found. Set JAVA_HOME or install Java from your operating system package manager.</p>}</section>
        <section className="settings-panel wide-panel"><div className="settings-panel-header"><div className="info-card-icon blue"><HardDrive size={17} /></div><div><strong>Minecraft locations</strong><span>{installations.length} local installation{installations.length === 1 ? "" : "s"} detected</span></div></div>{installations.map((installation) => <div className="runtime-row" key={installation.root}><span className="runtime-mark"><HardDrive size={14} /></span><div><strong>{installation.versions.length} installed version{installation.versions.length === 1 ? "" : "s"}</strong><small>{installation.root}</small></div></div>)}<button className="secondary-button" type="button" onClick={refreshInstallations}>Rescan locations</button></section>
        <section className="settings-panel wide-panel"><div className="settings-panel-header"><div className="info-card-icon amber"><MonitorCog size={17} /></div><div><strong>Application updates</strong><span>Update checks are not configured in this development build.</span></div></div><p className="settings-explainer">No update server or background check is enabled.</p></section>
      </div>
    </>
  );
}

export default App;
