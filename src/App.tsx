import { useEffect, useMemo, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import packageJson from "../package.json";
import { DownloadsPage } from "./pages/DownloadsPage";
import { VersionsPage } from "./pages/VersionsPage";
import {
  ArrowRight,
  ArrowDownToLine,
  Boxes,
  Check,
  ChevronDown,
  CircleAlert,
  CircleCheck,
  Clock3,
  Cpu,
  FolderOpen,
  Gamepad2,
  HardDrive,
  House,
  Layers3,
  LayoutGrid,
  List,
  LoaderCircle,
  Moon,
  MonitorCog,
  PackageOpen,
  Palette,
  Play,
  Plus,
  Search,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  Star,
  Sun,
  TerminalSquare,
  UsersRound,
  X,
  Zap,
} from "lucide-react";
import { formatRelativeTime, javaLabel } from "./format";
import type {
  Instance,
  JavaRuntime,
  LaunchExitEvent,
  LaunchLogEvent,
  LaunchStarted,
  MinecraftInstallation,
  OfflineProfile,
  DownloadProgress,
  RemoteMinecraftVersion,
} from "./types";

type ThemeMode = "dark" | "light";
type AccentColor = "lime" | "mint" | "sky" | "violet";
type InstanceFilter = "all" | "favorites";
type InstanceView = "grid" | "list";

const navigation = [
  { id: "home", label: "Overview", icon: House, group: "PLAY" },
  { id: "instances", label: "Instances", icon: Boxes, group: "LIBRARY" },
  { id: "versions", label: "Versions", icon: Layers3, group: "LIBRARY" },
  { id: "mods", label: "Mods", icon: PackageOpen, group: "LIBRARY" },
  { id: "modpacks", label: "Modpacks", icon: FolderOpen, group: "LIBRARY" },
  { id: "accounts", label: "Accounts", icon: UsersRound, group: "TOOLS" },
  { id: "downloads", label: "Downloads", icon: ArrowDownToLine, group: "TOOLS" },
  { id: "settings", label: "Settings", icon: Settings2, group: "TOOLS" },
] as const;

const navigationGroups = ["PLAY", "LIBRARY", "TOOLS"] as const;
const accentColors: { id: AccentColor; label: string }[] = [
  { id: "lime", label: "Citrus" },
  { id: "mint", label: "Mint" },
  { id: "sky", label: "Sky" },
  { id: "violet", label: "Violet" },
];

function readStoredValue(key: string): string | null {
  if (typeof window === "undefined") return null;
  try {
    return window.localStorage.getItem(`opnlauncher.${key}`);
  } catch {
    return null;
  }
}

function readStoredIds(key: string): string[] {
  try {
    const parsed: unknown = JSON.parse(readStoredValue(key) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === "string") : [];
  } catch {
    return [];
  }
}

function writeStoredValue(key: string, value: string): void {
  try {
    window.localStorage.setItem(`opnlauncher.${key}`, value);
  } catch {
    // Preferences are optional; the launcher remains usable if storage is unavailable.
  }
}

function removeStoredValue(key: string): void {
  try {
    window.localStorage.removeItem(`opnlauncher.${key}`);
  } catch {
    // Preferences are optional; the launcher remains usable if storage is unavailable.
  }
}

function formatMemory(memoryMb: number): string {
  const gigabytes = memoryMb / 1024;
  return `${Number.isInteger(gigabytes) ? gigabytes : gigabytes.toFixed(1)} GB`;
}

const appVersion = packageJson.version;

function App() {
  const desktop = isTauri();
  const [activePage, setActivePage] = useState<string>("home");
  const [instances, setInstances] = useState<Instance[]>([]);
  const [installations, setInstallations] = useState<MinecraftInstallation[]>([]);
  const [runtimes, setRuntimes] = useState<JavaRuntime[]>([]);
  const [profiles, setProfiles] = useState<OfflineProfile[]>([]);
  const [selectedInstanceId, setSelectedInstanceId] = useState<string | null>(() => readStoredValue("selected-instance"));
  const [selectedProfileId, setSelectedProfileId] = useState(() => readStoredValue("selected-profile") ?? "");
  const [favoriteIds, setFavoriteIds] = useState<string[]>(() => readStoredIds("favorite-instances"));
  const [theme, setTheme] = useState<ThemeMode>(() => readStoredValue("theme") === "light" ? "light" : "dark");
  const [accent, setAccent] = useState<AccentColor>(() => {
    const stored = readStoredValue("accent");
    return accentColors.some((color) => color.id === stored) ? stored as AccentColor : "lime";
  });
  const [selectedJavaPath, setSelectedJavaPath] = useState("");
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [showVersionInstallDialog, setShowVersionInstallDialog] = useState(false);
  const [showProfileForm, setShowProfileForm] = useState(false);
  const [instanceName, setInstanceName] = useState("");
  const [versionInstanceName, setVersionInstanceName] = useState("");
  const [profileName, setProfileName] = useState("");
  const [installPath, setInstallPath] = useState("");
  const [inspectedInstall, setInspectedInstall] = useState<MinecraftInstallation | null>(null);
  const [versionChoice, setVersionChoice] = useState("");
  const [memoryChoice, setMemoryChoice] = useState(2048);
  const [instanceSearch, setInstanceSearch] = useState("");
  const [instanceFilter, setInstanceFilter] = useState<InstanceFilter>("all");
  const [instanceView, setInstanceView] = useState<InstanceView>("grid");
  const [isBusy, setIsBusy] = useState(false);
  const [isSavingSettings, setIsSavingSettings] = useState(false);
  const [busyMessage, setBusyMessage] = useState("");
  const [error, setError] = useState("");
  const [logs, setLogs] = useState<string[]>([]);
  const [launchState, setLaunchState] = useState<"idle" | "running" | "exited">("idle");
  const [logPath, setLogPath] = useState("");
  const [officialVersions, setOfficialVersions] = useState<RemoteMinecraftVersion[]>([]);
  const [versionsLoading, setVersionsLoading] = useState(false);
  const [versionsError, setVersionsError] = useState("");
  const [selectedRemoteVersion, setSelectedRemoteVersion] = useState<RemoteMinecraftVersion | null>(null);
  const [installError, setInstallError] = useState("");
  const [downloadProgress, setDownloadProgress] = useState<DownloadProgress | null>(null);
  const [activeDownloadId, setActiveDownloadId] = useState<string | null>(null);
  const [isInstallingVersion, setIsInstallingVersion] = useState(false);
  const [showRenameDialog, setShowRenameDialog] = useState(false);
  const [showForgetDialog, setShowForgetDialog] = useState(false);
  const [renameValue, setRenameValue] = useState("");
  const activeDownloadIdRef = useRef<string | null>(null);
  const selectedInstanceIdRef = useRef(selectedInstanceId);
  const globalSearchRef = useRef<HTMLInputElement>(null);
  selectedInstanceIdRef.current = selectedInstanceId;

  const selectedInstance = instances.find((instance) => instance.id === selectedInstanceId) ?? null;
  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId) ?? null;
  const selectedJava = runtimes.find((runtime) => runtime.path === selectedJavaPath)
    ?? (selectedJavaPath === "" ? runtimes[0] ?? null : null);
  const downloadPercent = downloadProgress
    ? downloadProgress.totalBytes > 0
      ? Math.min(100, Math.round((downloadProgress.downloadedBytes / downloadProgress.totalBytes) * 100))
      : downloadProgress.totalFiles > 0
        ? Math.round((downloadProgress.completedFiles / downloadProgress.totalFiles) * 100)
        : 0
    : 0;
  const latestLogLines = useMemo(() => logs.slice(-100), [logs]);
  const visibleInstances = useMemo(() => {
    const query = instanceSearch.trim().toLocaleLowerCase();
    return [...instances]
      .filter((instance) => instanceFilter === "all" || favoriteIds.includes(instance.id))
      .filter((instance) => !query || `${instance.name} ${instance.versionId ?? ""} ${instance.gameDir}`.toLocaleLowerCase().includes(query))
      .sort((left, right) => {
        const favoriteOrder = Number(favoriteIds.includes(right.id)) - Number(favoriteIds.includes(left.id));
        return favoriteOrder || (right.lastPlayedAt ?? right.createdAt) - (left.lastPlayedAt ?? left.createdAt);
      });
  }, [favoriteIds, instanceFilter, instanceSearch, instances]);
  const recentInstances = useMemo(
    () => [...instances].sort((left, right) => (right.lastPlayedAt ?? right.createdAt) - (left.lastPlayedAt ?? left.createdAt)).slice(0, 4),
    [instances],
  );

  useEffect(() => {
    if (!desktop) return;
    let cancelled = false;
    let unlistenLog: UnlistenFn | undefined;
    let unlistenExit: UnlistenFn | undefined;
    let unlistenDownload: UnlistenFn | undefined;

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
        setSelectedInstanceId((current) => current && savedInstances.some((instance) => instance.id === current) ? current : savedInstances[0]?.id ?? null);
        if (savedInstances.length > 0) setMemoryChoice(savedInstances[0].memoryMb);
        setFavoriteIds((current) => current.filter((id) => savedInstances.some((instance) => instance.id === id)));
        if (foundRuntimes.length > 0) setSelectedJavaPath(foundRuntimes[0].path);
        const savedProfiles = await invoke<OfflineProfile[]>("list_offline_profiles");
        if (!cancelled) {
          setProfiles(savedProfiles);
          setSelectedProfileId((current) => savedProfiles.some((profile) => profile.id === current) ? current : savedProfiles[0]?.id ?? "");
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
    void listen<DownloadProgress>("download-progress", (event) => {
      if (event.payload.downloadId === activeDownloadIdRef.current) {
        setDownloadProgress(event.payload);
      }
    }).then((dispose) => {
      unlistenDownload = dispose;
    });
    return () => {
      cancelled = true;
      unlistenLog?.();
      unlistenExit?.();
      unlistenDownload?.();
    };
    // The selected instance ref keeps event subscriptions stable while selection changes.
  }, [desktop]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.accent = accent;
    writeStoredValue("theme", theme);
    writeStoredValue("accent", accent);
  }, [accent, theme]);

  useEffect(() => {
    if (selectedInstanceId) writeStoredValue("selected-instance", selectedInstanceId);
    else removeStoredValue("selected-instance");
  }, [selectedInstanceId]);

  useEffect(() => {
    if (selectedProfileId) writeStoredValue("selected-profile", selectedProfileId);
    else removeStoredValue("selected-profile");
  }, [selectedProfileId]);

  useEffect(() => {
    writeStoredValue("favorite-instances", JSON.stringify(favoriteIds));
  }, [favoriteIds]);

  useEffect(() => {
    function handleShortcut(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "k") {
        event.preventDefault();
        setActivePage("instances");
        window.requestAnimationFrame(() => globalSearchRef.current?.focus());
      }
    }
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, []);

  useEffect(() => {
    if (selectedInstance) {
      setMemoryChoice(selectedInstance.memoryMb);
      setSelectedJavaPath(selectedInstance.javaPath ?? runtimes[0]?.path ?? "");
    }
  }, [selectedInstanceId, instances, runtimes]);

  useEffect(() => {
    if (desktop && activePage === "versions" && officialVersions.length === 0) {
      void loadOfficialVersions();
    }
  }, [desktop, activePage]);

  async function refreshInstances() {
    const savedInstances = await invoke<Instance[]>("list_instances");
    setInstances(savedInstances);
  }

  async function refreshInstallations() {
    const found = await invoke<MinecraftInstallation[]>("scan_installations");
    setInstallations(found);
  }

  async function loadOfficialVersions() {
    setVersionsLoading(true);
    setVersionsError("");
    try {
      const found = await invoke<RemoteMinecraftVersion[]>("list_official_versions");
      setOfficialVersions(found);
    } catch (reason) {
      setVersionsError(String(reason));
    } finally {
      setVersionsLoading(false);
    }
  }

  function chooseRemoteVersion(version: RemoteMinecraftVersion) {
    setSelectedRemoteVersion(version);
    setVersionInstanceName(`Minecraft ${version.id}`);
    setInstallError("");
    setShowVersionInstallDialog(true);
  }

  async function installRemoteVersion(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedRemoteVersion || !versionInstanceName.trim() || isInstallingVersion) return;
    const downloadId = crypto.randomUUID();
    activeDownloadIdRef.current = downloadId;
    setActiveDownloadId(downloadId);
    setDownloadProgress({
      downloadId,
      versionId: selectedRemoteVersion.id,
      phase: "Preparing version metadata",
      currentFile: "",
      completedFiles: 0,
      totalFiles: 0,
      downloadedBytes: 0,
      totalBytes: 0,
    });
    setInstallError("");
    setIsInstallingVersion(true);
    try {
      const created = await invoke<Instance>("install_minecraft_version", {
        name: versionInstanceName.trim(),
        versionId: selectedRemoteVersion.id,
        downloadId,
      });
      setInstances((current) => [created, ...current]);
      setSelectedInstanceId(created.id);
      setMemoryChoice(created.memoryMb);
      setDownloadProgress((current) => current ? { ...current, phase: "Installation complete" } : current);
      setShowVersionInstallDialog(false);
      setSelectedRemoteVersion(null);
      setActivePage("instances");
    } catch (reason) {
      const message = String(reason);
      setInstallError(message);
      setDownloadProgress((current) => current ? {
        ...current,
        phase: message.toLowerCase().includes("cancel") ? "Installation cancelled" : "Installation failed",
      } : current);
    } finally {
      activeDownloadIdRef.current = null;
      setActiveDownloadId(null);
      setIsInstallingVersion(false);
    }
  }

  async function cancelDownload() {
    if (!activeDownloadId) return;
    try {
      await invoke("cancel_download", { downloadId: activeDownloadId });
    } catch (reason) {
      setInstallError(String(reason));
    }
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
    setIsSavingSettings(true);
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
    } finally {
      setIsSavingSettings(false);
    }
  }

  async function renameSelectedInstance(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!selectedInstance) return;
    const name = renameValue.trim();
    if (!name || name === selectedInstance.name) {
      setShowRenameDialog(false);
      return;
    }
    setError("");
    try {
      await invoke("rename_instance", { instanceId: selectedInstance.id, name });
      await refreshInstances();
      setShowRenameDialog(false);
    } catch (reason) {
      setError(String(reason));
    }
  }

  async function forgetSelectedInstance() {
    if (!selectedInstance) return;
    setError("");
    try {
      await invoke("forget_instance", { instanceId: selectedInstance.id, confirmed: true });
      const remaining = instances.filter((instance) => instance.id !== selectedInstance.id);
      setFavoriteIds((current) => current.filter((id) => id !== selectedInstance.id));
      setInstances(remaining);
      setSelectedInstanceId(remaining[0]?.id ?? null);
      setShowForgetDialog(false);
    } catch (reason) {
      setError(String(reason));
    }
  }

  function toggleFavorite(instanceId: string) {
    setFavoriteIds((current) => current.includes(instanceId)
      ? current.filter((id) => id !== instanceId)
      : [...current, instanceId]);
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
  const canLaunch = desktop && Boolean(selectedInstance?.versionId && selectedProfile && selectedJava)
    && launchState !== "running" && !isBusy;
  const launchHint = !desktop ? "Open the desktop app to launch Minecraft."
    : !selectedInstance ? "Choose or install an instance to get started."
      : !selectedInstance.versionId ? "Install a Minecraft version for this instance first."
        : !selectedProfile ? "Create a local developer profile for testing."
          : !selectedJava ? "Select a Java runtime before launching."
            : launchState === "running" ? "Minecraft is running."
              : isBusy ? "The launcher is working."
                : "Ready to launch.";
  const versionedInstanceCount = instances.filter((instance) => instance.versionId).length;

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark"><Gamepad2 size={21} strokeWidth={2.1} /></div>
          <div className="brand-copy"><strong>OpnLauncher</strong><span>YOUR WORLDS, YOUR WAY</span></div>
        </div>

        <button className="sidebar-profile" type="button" onClick={() => setActivePage("accounts")} title="Manage local developer profiles">
          <span className="profile-avatar">{selectedProfile?.name.slice(0, 1).toUpperCase() ?? <UsersRound size={16} />}</span>
          <span className="sidebar-profile-copy"><strong>{selectedProfile?.name ?? "Set up a profile"}</strong><small>UNVERIFIED · LOCAL</small></span>
          <ChevronDown size={15} />
        </button>

        <div className="sidebar-navigation">
          {navigationGroups.map((group) => (
            <div className="nav-group" key={group}>
              <div className="nav-label">{group}</div>
              <nav className="main-nav" aria-label={`${group.toLocaleLowerCase()} navigation`}>
                {navigation.filter((item) => item.group === group).map(({ id, label, icon: Icon }) => (
                  <button
                    className={`nav-item ${activePage === id ? "is-active" : ""}`}
                    key={id}
                    onClick={() => setActivePage(id)}
                    type="button"
                    aria-current={activePage === id ? "page" : undefined}
                  >
                    <Icon size={18} strokeWidth={1.8} />
                    <span>{label}</span>
                    {id === "instances" && instances.length > 0 && <span className="nav-count">{instances.length}</span>}
                    {id === "downloads" && isInstallingVersion && <span className="nav-dot" aria-label="Download in progress" />}
                  </button>
                ))}
              </nav>
            </div>
          ))}
        </div>

        <div className="sidebar-bottom">
          <div className="privacy-card">
            <div className="privacy-icon"><ShieldCheck size={16} /></div>
            <div><strong>Private by default</strong><span>Your launcher data stays here.</span></div>
          </div>
          <div className="sidebar-version"><span className="version-mark">OP</span><span>{appVersion}</span><span className="dev-badge">DEV</span></div>
        </div>
      </aside>

      <main className="main-content">
        <header className="topbar">
          <div className="breadcrumbs"><span>Workspace</span><ArrowRight size={13} /><strong>{activeNavigation.label}</strong></div>
          <div className="topbar-actions">
            <label className="global-search">
              <Search size={16} />
              <input
                ref={globalSearchRef}
                aria-label="Search instances"
                placeholder="Find an instance"
                value={instanceSearch}
                onFocus={() => setActivePage("instances")}
                onChange={(event) => { setInstanceSearch(event.target.value); setActivePage("instances"); }}
              />
              <kbd>Ctrl K</kbd>
            </label>
            <button className="topbar-new-button" type="button" onClick={() => setShowAddDialog(true)}><Plus size={16} /><span>New instance</span></button>
            <button className="topbar-account" type="button" onClick={() => setActivePage("accounts")} aria-label="Open accounts">
              <span className="profile-avatar">{selectedProfile?.name.slice(0, 1).toUpperCase() ?? <UsersRound size={15} />}</span>
              <ChevronDown size={14} />
            </button>
          </div>
        </header>

        <div className="page-scroll">
          {error && <div className="error-banner" role="alert"><CircleAlert size={18} /><span>{error}</span><button type="button" onClick={() => setError("")} aria-label="Dismiss error"><X size={16} /></button></div>}
          {!desktop && <div className="notice-banner"><CircleAlert size={18} /><span>Desktop features are available when OpnLauncher is opened as a desktop app.</span></div>}

          {activePage === "home" ? (
            <>
              <section className="page-heading">
                <div><p className="eyebrow"><Sparkles size={14} /> YOUR LOCAL GAME SPACE</p><h1>Let’s play.</h1><p className="page-subtitle">A calm home for every instance, version, and test profile.</p></div>
                <div className="device-status"><span className="status-pip" /> Local device <span className="status-divider" /> {runtimes.length} Java runtime{runtimes.length === 1 ? "" : "s"}</div>
              </section>

              <div className="home-grid">
                <section className="hero-card">
                  <div className="hero-art" aria-hidden="true"><span className="hero-sun" /><span className="hero-ridge ridge-back" /><span className="hero-ridge ridge-front" /><span className="hero-pixel pixel-one" /><span className="hero-pixel pixel-two" /><span className="hero-pixel pixel-three" /></div>
                  <div className="hero-content">
                    <div className="hero-topline"><span className="hero-kicker"><Zap size={14} /> QUICK PLAY</span><span className="hero-local-tag"><span /> LOCAL INSTANCE</span></div>
                    {selectedInstance ? (
                      <>
                        <div className="hero-version"><Layers3 size={15} /> {selectedInstance.versionId ?? "No version installed"}</div>
                        <h2>{selectedInstance.name}</h2>
                        <p className="hero-description">Your selected world is ready whenever you are.</p>
                        <div className="hero-facts">
                          <span><Cpu size={15} /> {selectedJava ? javaLabel(selectedJava.majorVersion) : "Java not selected"}</span>
                          <span><HardDrive size={15} /> {formatMemory(memoryChoice)}</span>
                          <span><Clock3 size={15} /> {formatRelativeTime(selectedInstance.lastPlayedAt)}</span>
                        </div>
                        <div className="hero-actions">
                          <button className="play-button" type="button" disabled={!canLaunch} onClick={() => void launchSelected()}>
                            {launchState === "running" ? <LoaderCircle className="spin" size={18} /> : <Play size={17} fill="currentColor" />}
                            {launchState === "running" ? "Launching…" : "Play now"}
                          </button>
                          <button className="hero-secondary-button" type="button" onClick={() => setActivePage("instances")}><Settings2 size={16} /> Instance settings</button>
                          <button className={`favorite-toggle hero-favorite ${favoriteIds.includes(selectedInstance.id) ? "is-favorite" : ""}`} type="button" onClick={() => toggleFavorite(selectedInstance.id)} aria-label={favoriteIds.includes(selectedInstance.id) ? "Remove from favorites" : "Add to favorites"} aria-pressed={favoriteIds.includes(selectedInstance.id)}><Star size={17} fill={favoriteIds.includes(selectedInstance.id) ? "currentColor" : "none"} /></button>
                        </div>
                        <p className={`launch-hint ${canLaunch ? "is-ready" : ""}`}><span /> {launchHint}</p>
                      </>
                    ) : (
                      <div className="hero-empty">
                        <h2>Your next world awaits.</h2>
                        <p>Install an official Minecraft version or connect an existing game folder to create your first instance.</p>
                        <div className="hero-actions"><button className="play-button" type="button" onClick={() => setActivePage("versions")}><ArrowDownToLine size={17} /> Browse versions</button><button className="hero-secondary-button" type="button" onClick={() => setShowAddDialog(true)}><Plus size={16} /> Add existing files</button></div>
                      </div>
                    )}
                  </div>
                </section>

                <aside className="home-rail">
                  <section className="account-summary-card">
                    <div className="card-kicker"><UsersRound size={15} /> TEST PROFILE</div>
                    <button className="account-summary-main" type="button" onClick={() => setActivePage("accounts")}>
                      <span className="account-large-avatar">{selectedProfile?.name.slice(0, 1).toUpperCase() ?? <UsersRound size={20} />}</span>
                      <span><strong>{selectedProfile?.name ?? "Create a local profile"}</strong><small>{selectedProfile ? "Unverified · local only" : "Needed for local test launches"}</small></span>
                      <ChevronDown size={16} />
                    </button>
                    <div className="account-local-note"><ShieldCheck size={15} /><span>No password or Microsoft sign-in. Profile data stays on this device.</span></div>
                    <button className="text-link" type="button" onClick={() => setActivePage("accounts")}>{profiles.length ? "Manage profiles" : "Create test profile"}<ArrowRight size={14} /></button>
                  </section>
                  <section className="device-card">
                    <div className="device-card-heading"><span className="device-card-icon"><MonitorCog size={17} /></span><span><strong>This device</strong><small>Local setup overview</small></span></div>
                    <div className="device-stat-row"><span>Instances</span><strong>{instances.length}</strong></div>
                    <div className="device-stat-row"><span>Versioned instances</span><strong>{versionedInstanceCount}</strong></div>
                    <div className="device-stat-row"><span>Java runtimes</span><strong>{runtimes.length}</strong></div>
                    <button className="device-settings-link" type="button" onClick={() => setActivePage("settings")}>Review setup <ArrowRight size={14} /></button>
                  </section>
                </aside>
              </div>

              <section className="section-block">
                <div className="section-heading"><div><p className="eyebrow">PICK UP WHERE YOU LEFT OFF</p><h2>Recent instances</h2></div><button className="text-link" type="button" onClick={() => setActivePage("instances")}>View library <ArrowRight size={15} /></button></div>
                {recentInstances.length > 0 ? (
                  <div className="home-instance-grid">
                    {recentInstances.map((instance, index) => (
                      <article className={`home-instance-card ${selectedInstanceId === instance.id ? "selected" : ""}`} key={instance.id}>
                        <button className={`home-instance-main art-${index % 4}`} type="button" onClick={() => setSelectedInstanceId(instance.id)} aria-label={`Select ${instance.name}`}>
                          <span className="instance-art-icon"><Gamepad2 size={20} /></span><span className="instance-version-chip">{instance.versionId ?? "Setup needed"}</span>
                        </button>
                        <div className="home-instance-info"><button type="button" className="instance-title-button" onClick={() => setSelectedInstanceId(instance.id)}><strong>{instance.name}</strong><small>{formatRelativeTime(instance.lastPlayedAt)}</small></button><button className={`favorite-toggle ${favoriteIds.includes(instance.id) ? "is-favorite" : ""}`} type="button" onClick={() => toggleFavorite(instance.id)} aria-label={favoriteIds.includes(instance.id) ? "Remove from favorites" : "Add to favorites"} aria-pressed={favoriteIds.includes(instance.id)}><Star size={16} fill={favoriteIds.includes(instance.id) ? "currentColor" : "none"} /></button></div>
                      </article>
                    ))}
                    <button className="add-instance-tile" type="button" onClick={() => setShowAddDialog(true)}><span><Plus size={18} /></span><strong>New instance</strong><small>Download or connect a game</small></button>
                  </div>
                ) : (
                  <div className="empty-library-card"><div className="empty-icon"><Boxes size={23} /></div><div><strong>Your library is empty</strong><span>Start with a Minecraft version download or add local files.</span></div><button className="play-button" type="button" onClick={() => setActivePage("versions")}><ArrowDownToLine size={16} /> Browse versions</button></div>
                )}
              </section>

              {launchState !== "idle" && (
                <section className="console-panel">
                  <div className="console-heading"><div><TerminalSquare size={17} /><strong>Game session output</strong><span className={`console-status ${launchState}`}>{launchState === "running" ? "RUNNING" : "CLOSED"}</span></div><span>{logPath}</span></div>
                  <div className="console-body" role="log" aria-live="polite">{latestLogLines.map((line, index) => <code key={`${index}-${line}`}>{line}</code>)}</div>
                </section>
              )}

              <section className="shortcut-row">
                <button type="button" onClick={() => setActivePage("versions")}><span className="shortcut-icon accent-version"><Layers3 size={18} /></span><span><strong>Explore versions</strong><small>Releases, snapshots, and classics</small></span><ArrowRight size={16} /></button>
                <button type="button" onClick={() => setActivePage("downloads")}><span className="shortcut-icon accent-download"><ArrowDownToLine size={18} /></span><span><strong>Download activity</strong><small>{isInstallingVersion ? "An installation is in progress" : "Review your latest install"}</small></span><ArrowRight size={16} /></button>
                <button type="button" onClick={() => setActivePage("accounts")}><span className="shortcut-icon accent-account"><UsersRound size={18} /></span><span><strong>Test profiles</strong><small>{profiles.length} local profile{profiles.length === 1 ? "" : "s"} saved here</small></span><ArrowRight size={16} /></button>
              </section>
            </>
          ) : activePage === "instances" ? (
            <>
              <section className="page-heading page-heading-row">
                <div><p className="eyebrow"><Boxes size={14} /> YOUR LIBRARY</p><h1>Instances</h1><p className="page-subtitle">Separate game folders, settings, and worlds for every setup.</p></div>
                <div className="heading-actions"><button className="secondary-button" type="button" onClick={() => void refreshInstallations()}><FolderOpen size={16} /> Scan local files</button><button className="play-button" type="button" onClick={() => setShowAddDialog(true)}><Plus size={16} /> Add instance</button></div>
              </section>
              <div className="library-overview">
                <button type="button" className={instanceFilter === "all" ? "selected" : ""} onClick={() => setInstanceFilter("all")}><span>All instances</span><strong>{instances.length}</strong></button>
                <button type="button" className={instanceFilter === "favorites" ? "selected" : ""} onClick={() => setInstanceFilter("favorites")}><span>Favorites</span><strong>{favoriteIds.length}</strong></button>
                <button type="button" onClick={() => setActivePage("versions")}><span>Install a version</span><ArrowRight size={16} /></button>
              </div>
              <div className="instance-toolbar">
                <label className="search-box"><Search size={17} /><input aria-label="Search instances" placeholder="Search by name, version, or folder" value={instanceSearch} onChange={(event) => setInstanceSearch(event.target.value)} /><kbd>/</kbd></label>
                <div className="view-switch" role="group" aria-label="Instance layout">
                  <button type="button" className={instanceView === "grid" ? "selected" : ""} onClick={() => setInstanceView("grid")} aria-label="Grid view"><LayoutGrid size={17} /></button>
                  <button type="button" className={instanceView === "list" ? "selected" : ""} onClick={() => setInstanceView("list")} aria-label="List view"><List size={18} /></button>
                </div>
              </div>
              {instances.length === 0 ? (
                <div className="empty-library-card empty-library-large"><div className="empty-icon"><Gamepad2 size={23} /></div><div><strong>Nothing in your library yet</strong><span>Install a version from Mojang or register a game folder already on this device.</span></div><div className="empty-actions"><button className="play-button" type="button" onClick={() => setActivePage("versions")}><ArrowDownToLine size={16} /> Browse versions</button><button className="secondary-button" type="button" onClick={() => setShowAddDialog(true)}>Add local files</button></div></div>
              ) : visibleInstances.length === 0 ? (
                <div className="empty-library-card empty-library-large"><div className="empty-icon"><Search size={22} /></div><div><strong>{instanceFilter === "favorites" && favoriteIds.length === 0 ? "No favorites yet" : "No matches found"}</strong><span>{instanceFilter === "favorites" && favoriteIds.length === 0 ? "Mark an instance with a star and it will appear here." : "Try another search or clear the current filter."}</span></div><button className="secondary-button" type="button" onClick={() => { setInstanceSearch(""); setInstanceFilter("all"); }}>Clear filters</button></div>
              ) : (
                <div className={`instances-workspace ${instanceView === "list" ? "list-view" : "grid-view"}`}>
                  <div className={`instance-collection ${instanceView === "list" ? "as-list" : "as-grid"}`}>
                    {visibleInstances.map((instance, index) => (
                      <article className={`library-instance-card ${selectedInstanceId === instance.id ? "selected" : ""}`} key={instance.id}>
                        <button className={`library-instance-art art-${index % 4}`} type="button" onClick={() => setSelectedInstanceId(instance.id)} aria-label={`Select ${instance.name}`}><Gamepad2 size={20} /><span className="art-grid-overlay" /></button>
                        <div className="library-instance-body">
                          <div className="library-instance-heading"><button type="button" className="instance-title-button" onClick={() => setSelectedInstanceId(instance.id)}><strong>{instance.name}</strong><small>{instance.versionId ?? "No version installed"}</small></button><button className={`favorite-toggle ${favoriteIds.includes(instance.id) ? "is-favorite" : ""}`} type="button" onClick={() => toggleFavorite(instance.id)} aria-label={favoriteIds.includes(instance.id) ? "Remove from favorites" : "Add to favorites"} aria-pressed={favoriteIds.includes(instance.id)}><Star size={16} fill={favoriteIds.includes(instance.id) ? "currentColor" : "none"} /></button></div>
                          <div className="library-instance-meta"><span><Clock3 size={13} /> {formatRelativeTime(instance.lastPlayedAt)}</span><span className={`install-state ${instance.versionId ? "ready" : "needs-setup"}`}><span /> {instance.versionId ? "Installed" : "Setup needed"}</span></div>
                        </div>
                      </article>
                    ))}
                    <button className="add-instance-tile library-add-tile" type="button" onClick={() => setShowAddDialog(true)}><span><Plus size={18} /></span><strong>Add an instance</strong><small>Use a download or local files</small></button>
                  </div>

                  {selectedInstance && (
                    <section className="instance-settings-card">
                      <div className="details-card-topline"><span>INSTANCE SETTINGS</span><button className={`favorite-toggle ${favoriteIds.includes(selectedInstance.id) ? "is-favorite" : ""}`} type="button" onClick={() => toggleFavorite(selectedInstance.id)} aria-label={favoriteIds.includes(selectedInstance.id) ? "Remove from favorites" : "Add to favorites"} aria-pressed={favoriteIds.includes(selectedInstance.id)}><Star size={17} fill={favoriteIds.includes(selectedInstance.id) ? "currentColor" : "none"} /></button></div>
                      <div className="details-instance-title"><span className="details-game-icon"><Gamepad2 size={22} /></span><span><strong>{selectedInstance.name}</strong><small>{selectedInstance.versionId ?? "No game version selected"}</small></span></div>
                      <div className="details-divider" />
                      <label className="field-label" htmlFor="java-path">Java runtime</label>
                      <div className="select-wrap"><select id="java-path" className="field-control" value={selectedJavaPath} onChange={(event) => setSelectedJavaPath(event.target.value)}><option value="">Automatic detection</option>{runtimes.map((runtime) => <option key={runtime.path} value={runtime.path}>Java {runtime.majorVersion} · {runtime.vendor ?? "Runtime"}</option>)}</select><ChevronDown size={16} /></div>
                      {selectedJava ? <div className="runtime-description"><span className="runtime-dot" /><span><strong>{selectedJavaPath ? selectedJava.version : `Automatic · ${selectedJava.version}`}</strong><small title={selectedJava.path}>{selectedJava.path}</small></span><span className="runtime-architecture">{selectedJava.architecture ?? "Unknown"}</span></div> : <div className="inline-warning"><CircleAlert size={15} /><span>No Java runtime detected. Set JAVA_HOME, then refresh this page.</span></div>}

                      <div className="memory-heading"><div><label className="field-label" htmlFor="memory">Memory allocation</label><small>Maximum RAM available to Minecraft</small></div><strong>{formatMemory(memoryChoice)}</strong></div>
                      <input id="memory" className="memory-slider" type="range" min="1024" max="16384" step="512" value={memoryChoice} onChange={(event) => setMemoryChoice(Number(event.target.value))} aria-valuetext={formatMemory(memoryChoice)} style={{ background: `linear-gradient(90deg, var(--accent) 0%, var(--accent) ${((memoryChoice - 1024) / (16384 - 1024)) * 100}%, var(--line) ${((memoryChoice - 1024) / (16384 - 1024)) * 100}%, var(--line) 100%)` }} />
                      <div className="slider-scale"><span>1 GB</span><span>16 GB</span></div>

                      <div className="detail-path"><HardDrive size={15} /><span title={selectedInstance.gameDir}>{selectedInstance.gameDir}</span></div>
                      <button className="save-button" type="button" onClick={() => void saveInstanceSettings()} disabled={isSavingSettings}>{isSavingSettings ? <LoaderCircle className="spin" size={16} /> : <Check size={16} />}{isSavingSettings ? "Saving settings…" : "Save settings"}</button>
                      <button className="detail-launch-button" type="button" disabled={!canLaunch} onClick={() => void launchSelected()}><Play size={15} fill="currentColor" /> Launch this instance</button>
                      <div className="instance-action-links"><button type="button" onClick={() => { setRenameValue(selectedInstance.name); setShowRenameDialog(true); }}>Rename</button><button type="button" className="danger-link" onClick={() => setShowForgetDialog(true)}>Remove from library</button></div>
                      <p className="preserve-note">Removing an instance keeps its game files, saves, and worlds on disk.</p>
                    </section>
                  )}
                </div>
              )}
              <form className="quick-create" onSubmit={(event) => void createBlankInstance(event)}><div><strong>Need a clean folder?</strong><span>Create an empty instance, then install a version later.</span></div><input value={instanceName} onChange={(event) => setInstanceName(event.target.value)} placeholder="New instance name" aria-label="New instance name" maxLength={48} /><button type="submit" className="secondary-button" disabled={isBusy || !instanceName.trim()}><Plus size={15} /> Create empty</button></form>
            </>
          ) : activePage === "versions" ? (
            <VersionsPage versions={officialVersions} loading={versionsLoading} error={versionsError} onRefresh={() => void loadOfficialVersions()} onInstall={chooseRemoteVersion} installing={isInstallingVersion} desktop={desktop} />
          ) : activePage === "downloads" ? (
            <DownloadsPage progress={downloadProgress} active={isInstallingVersion} onCancel={() => void cancelDownload()} />
          ) : activePage === "accounts" ? (
            <AccountsPage profiles={profiles} selectedProfileId={selectedProfileId} onSelect={(profileId) => setSelectedProfileId(profileId)} onCreate={() => setShowProfileForm(true)} />
          ) : activePage === "settings" ? (
            <SettingsPage runtimes={runtimes} installations={installations} refreshInstallations={() => void refreshInstallations()} theme={theme} onThemeChange={setTheme} accent={accent} onAccentChange={setAccent} />
          ) : (
            <>
              <section className="page-heading"><div><p className="eyebrow"><PackageOpen size={14} /> MODDING WORKSPACE</p><h1>{activeNavigation.label}</h1><p className="page-subtitle">A focused place to manage {activePage === "mods" ? "mods and their required loaders" : "modpacks and their files"}.</p></div><span className="planned-badge"><span /> IN DEVELOPMENT</span></section>
              <div className="feature-preview-card"><div className="preview-icon"><PackageOpen size={22} /></div><div><span className="eyebrow">PLANNED INCREMENTALLY</span><h2>{activePage === "mods" ? "Your mod library starts here." : "Modpacks, kept safely separate."}</h2><p>{activePage === "mods" ? "Loader detection, compatibility checks, and trusted provider search are planned for a later development phase." : "Import, update, and export workflows will preserve local changes before they touch your files."}</p><button className="text-link" type="button" onClick={() => setActivePage("versions")}>Browse Minecraft versions <ArrowRight size={15} /></button></div></div>
            </>
          )}

          <footer className="page-footer"><span>OpnLauncher is an independent community project and is not affiliated with Mojang or Microsoft.</span><span>{appVersion}</span></footer>
        </div>
      </main>

      {isBusy && busyMessage && <div className="busy-toast"><LoaderCircle className="spin" size={17} /> {busyMessage}</div>}

      {showAddDialog && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowAddDialog(false); }}>
          <section className="modal-card add-instance-modal" role="dialog" aria-modal="true" aria-labelledby="add-modal-title">
            <div className="modal-header"><div><p className="eyebrow"><Plus size={14} /> BUILD YOUR LIBRARY</p><h2 id="add-modal-title">Add an instance</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setShowAddDialog(false)}><X size={18} /></button></div>
            <p className="modal-copy">Each instance gets its own folder, settings, and worlds. Existing game files are only registered; they are not moved.</p>
            <button className="version-download-callout" type="button" onClick={() => { setShowAddDialog(false); setActivePage("versions"); }} disabled={!desktop}><span className="version-note-icon"><ArrowDownToLine size={17} /></span><span><strong>Install an official version</strong><small>Choose a release, snapshot, or historical version.</small></span><ArrowRight size={17} /></button>
            <div className="modal-section-heading"><span>EXISTING INSTALLATIONS</span><button className="text-link" type="button" onClick={() => void refreshInstallations()}><FolderOpen size={14} /> Rescan</button></div>
            {installations.length > 0 ? <div className="detected-list">{installations.map((installation) => <div className="detected-install" key={installation.root}><div className="detected-root"><HardDrive size={16} /><span title={installation.root}>{installation.root}</span></div><div className="detected-version-list">{installation.versions.slice(0, 6).map((version) => <button className="version-option" key={version.id} type="button" onClick={() => void addDetectedInstallation(installation, version.id)}><span><strong>{version.id}</strong><small>{version.kind}</small></span><Plus size={16} /></button>)}</div></div>)}</div> : <div className="no-detected"><FolderOpen size={19} /><span>No standard installation found. You can still download a version above or inspect a folder below.</span></div>}
            <label className="field-label" htmlFor="custom-install-path">Inspect another game folder</label><div className="custom-path-row"><input id="custom-install-path" className="field-control" value={installPath} onChange={(event) => setInstallPath(event.target.value)} placeholder="Path to .minecraft" /><button className="secondary-button" type="button" onClick={() => void inspectCustomInstallation()} disabled={!installPath.trim() || isBusy}>Inspect</button></div>
            {inspectedInstall && <div className="inspected-version"><label className="field-label" htmlFor="version-choice">Choose installed version</label><select id="version-choice" className="field-control" value={versionChoice} onChange={(event) => setVersionChoice(event.target.value)}>{inspectedInstall.versions.map((version) => <option value={version.id} key={version.id}>{version.id} · {version.kind}</option>)}</select><button className="play-button" type="button" onClick={() => void addInspectedInstallation()}><Plus size={16} /> Add selected version</button></div>}
            <details className="empty-instance-details"><summary>Create an empty instance instead</summary><form onSubmit={(event) => void createBlankInstance(event)}><input className="field-control" value={instanceName} onChange={(event) => setInstanceName(event.target.value)} placeholder="Instance name" maxLength={48} /><button className="secondary-button" type="submit" disabled={isBusy || !instanceName.trim()}>Create</button></form></details>
          </section>
        </div>
      )}

      {showVersionInstallDialog && selectedRemoteVersion && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget && !isInstallingVersion) setShowVersionInstallDialog(false); }}>
          <section className="modal-card version-install-modal" role="dialog" aria-modal="true" aria-labelledby="version-install-title">
            <div className="modal-header"><div><p className="eyebrow"><Layers3 size={14} /> OFFICIAL MINECRAFT FILES</p><h2 id="version-install-title">Install {selectedRemoteVersion.id}</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setShowVersionInstallDialog(false)} disabled={isInstallingVersion}><X size={18} /></button></div>
            <p className="modal-copy">Files will be verified and installed into a new OpnLauncher instance folder. Your other Minecraft folders and worlds are left alone.</p>
            <form className="version-install-form" onSubmit={(event) => void installRemoteVersion(event)}><label className="field-label" htmlFor="version-instance-name">Instance name</label><input id="version-instance-name" className="field-control" value={versionInstanceName} onChange={(event) => setVersionInstanceName(event.target.value)} maxLength={48} required disabled={isInstallingVersion} />
              {installError && <div className="error-banner" role="alert"><CircleAlert size={16} /><span>{installError}</span></div>}
              {isInstallingVersion && downloadProgress && <div className="install-progress-card"><div><LoaderCircle className="spin" size={16} /><strong>{downloadProgress.phase}</strong><span>{downloadProgress.completedFiles} / {downloadProgress.totalFiles} files</span></div><div className="download-progress-track"><span style={{ width: `${downloadPercent}%` }} /></div>{downloadProgress.currentFile && <small>{downloadProgress.currentFile}</small>}</div>}
              <div className="version-install-actions">{isInstallingVersion ? <button className="secondary-button" type="button" onClick={() => void cancelDownload()}>Cancel download</button> : <button className="play-button" type="submit" disabled={!versionInstanceName.trim()}><ArrowDownToLine size={16} /> Install version</button>}</div>
            </form>
          </section>
        </div>
      )}

      {showProfileForm && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowProfileForm(false); }}>
          <section className="modal-card profile-modal" role="dialog" aria-modal="true" aria-labelledby="profile-modal-title">
            <div className="modal-header"><div><p className="eyebrow"><UsersRound size={14} /> LOCAL TESTING</p><h2 id="profile-modal-title">Developer profile</h2></div><button className="icon-button" type="button" aria-label="Close" onClick={() => setShowProfileForm(false)}><X size={18} /></button></div>
            <div className="profile-warning"><ShieldCheck size={17} /><span>Unverified local profile. It does not sign in to Microsoft, verify game ownership, or authenticate to online servers.</span></div>
            {profiles.length > 0 && <div className="saved-profiles">{profiles.map((profile) => <button className={`saved-profile ${selectedProfileId === profile.id ? "selected" : ""}`} key={profile.id} type="button" onClick={() => { setSelectedProfileId(profile.id); setShowProfileForm(false); }}><span className="profile-avatar">{profile.name.slice(0, 1).toUpperCase()}</span><span><strong>{profile.name}</strong><small>UNVERIFIED · LOCAL</small></span>{selectedProfileId === profile.id && <Check size={17} />}</button>)}</div>}
            <form className="profile-create-form" onSubmit={(event) => void createProfile(event)}><label className="field-label" htmlFor="profile-name">Test username</label><input id="profile-name" className="field-control" value={profileName} onChange={(event) => setProfileName(event.target.value)} placeholder="LocalPlayer" maxLength={16} pattern="[A-Za-z0-9_]{1,16}" required /><small>Use 1–16 letters, numbers, or underscores.</small><button className="play-button" type="submit"><Plus size={16} /> Create local profile</button></form>
          </section>
        </div>
      )}

      {showRenameDialog && selectedInstance && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowRenameDialog(false); }}>
          <section className="modal-card small-modal" role="dialog" aria-modal="true" aria-labelledby="rename-title"><div className="modal-header"><div><p className="eyebrow"><SlidersHorizontal size={14} /> INSTANCE SETTINGS</p><h2 id="rename-title">Rename instance</h2></div><button className="icon-button" type="button" onClick={() => setShowRenameDialog(false)} aria-label="Close"><X size={18} /></button></div><form className="dialog-form" onSubmit={(event) => void renameSelectedInstance(event)}><label className="field-label" htmlFor="rename-instance">Instance name</label><input id="rename-instance" className="field-control" value={renameValue} onChange={(event) => setRenameValue(event.target.value)} maxLength={48} required autoFocus /><div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setShowRenameDialog(false)}>Cancel</button><button className="play-button" type="submit" disabled={!renameValue.trim()}>Save name</button></div></form></section>
        </div>
      )}

      {showForgetDialog && selectedInstance && (
        <div className="modal-backdrop" role="presentation" onMouseDown={(event) => { if (event.target === event.currentTarget) setShowForgetDialog(false); }}>
          <section className="modal-card small-modal" role="alertdialog" aria-modal="true" aria-labelledby="forget-title"><div className="modal-header"><div><p className="eyebrow danger-eyebrow"><CircleAlert size={14} /> REMOVE FROM LIBRARY</p><h2 id="forget-title">Remove {selectedInstance.name}?</h2></div><button className="icon-button" type="button" onClick={() => setShowForgetDialog(false)} aria-label="Close"><X size={18} /></button></div><p className="modal-copy">This removes the instance entry from OpnLauncher. Its game files, saves, and worlds will stay on disk.</p><div className="dialog-actions"><button className="secondary-button" type="button" onClick={() => setShowForgetDialog(false)}>Keep instance</button><button className="danger-button" type="button" onClick={() => void forgetSelectedInstance()}>Remove from launcher</button></div></section>
        </div>
      )}
    </div>
  );
}

function AccountsPage({
  profiles,
  selectedProfileId,
  onSelect,
  onCreate,
}: {
  profiles: OfflineProfile[];
  selectedProfileId: string;
  onSelect: (profileId: string) => void;
  onCreate: () => void;
}) {
  const selectedProfile = profiles.find((profile) => profile.id === selectedProfileId) ?? null;

  return (
    <>
      <section className="page-heading page-heading-row">
        <div><p className="eyebrow"><UsersRound size={14} /> LOCAL TESTING</p><h1>Accounts</h1><p className="page-subtitle">Keep a separate, unverified profile ready for launcher and game testing.</p></div>
        <button className="play-button" type="button" onClick={onCreate}><Plus size={16} /> Create profile</button>
      </section>

      <div className="account-content-grid">
        <section className="account-manager-card">
          <div className="section-heading account-list-heading"><div><p className="eyebrow">SAVED ON THIS DEVICE</p><h2>Developer profiles</h2></div><span className="count-pill">{profiles.length}</span></div>
          <div className="profile-warning account-warning"><ShieldCheck size={18} /><span>These test profiles are always marked unverified. They do not sign in to Microsoft or verify Minecraft ownership.</span></div>
          {profiles.length > 0 ? <div className="account-profile-list">{profiles.map((profile) => (
            <article className={`account-profile-card ${selectedProfileId === profile.id ? "selected" : ""}`} key={profile.id}>
              <button className="account-profile-select" type="button" onClick={() => onSelect(profile.id)} aria-pressed={selectedProfileId === profile.id}>
                <span className="account-large-avatar">{profile.name.slice(0, 1).toUpperCase()}</span>
                <span><strong>{profile.name}</strong><small>UNVERIFIED · LOCAL TEST PROFILE</small></span>
                {selectedProfileId === profile.id ? <span className="active-profile-badge"><CircleCheck size={14} /> Selected</span> : <span className="select-profile-label">Use profile</span>}
              </button>
              {selectedProfileId === profile.id && <div className="profile-identifier"><span>Offline UUID</span><code>{profile.uuid}</code></div>}
            </article>
          ))}</div> : <div className="empty-profile-state"><span className="empty-icon"><UsersRound size={22} /></span><strong>No developer profiles yet</strong><span>Create a local username to test the launcher and single-player launch flow.</span><button className="secondary-button" type="button" onClick={onCreate}><Plus size={15} /> Create the first profile</button></div>}
          {profiles.length > 0 && <button className="secondary-button add-profile-button" type="button" onClick={onCreate}><Plus size={15} /> Add another profile</button>}
        </section>

        <aside className="account-boundaries-card">
          <div className="account-boundaries-icon"><ShieldCheck size={21} /></div>
          <p className="eyebrow">TESTING BOUNDARIES</p><h2>Made for local development.</h2>
          <p>Local profiles are useful for checking instances, settings, worlds, and single-player launches while account authentication is being built.</p>
          <div className="boundary-list"><div><CircleCheck size={16} /><span>Local single-player testing</span></div><div><CircleCheck size={16} /><span>Stable local profile identity</span></div><div className="boundary-restricted"><CircleAlert size={16} /><span>No Microsoft or online-server authentication</span></div></div>
          <div className="planned-signin"><span className="planned-badge"><span /> PLANNED</span><strong>Microsoft sign-in</strong><small>Official authentication will be added as a separate account type.</small></div>
          {selectedProfile && <div className="selected-profile-note">Current test profile: <strong>{selectedProfile.name}</strong></div>}
        </aside>
      </div>
    </>
  );
}

function SettingsPage({
  runtimes,
  installations,
  refreshInstallations,
  theme,
  onThemeChange,
  accent,
  onAccentChange,
}: {
  runtimes: JavaRuntime[];
  installations: MinecraftInstallation[];
  refreshInstallations: () => void;
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  accent: AccentColor;
  onAccentChange: (accent: AccentColor) => void;
}) {
  return (
    <>
      <section className="page-heading"><div><p className="eyebrow"><Settings2 size={14} /> PERSONALIZE YOUR LAUNCHER</p><h1>Settings</h1><p className="page-subtitle">Appearance, Java runtimes, and local game folders.</p></div></section>
      <div className="settings-layout">
        <section className="settings-card appearance-card">
          <div className="settings-card-heading"><span className="settings-icon"><Palette size={18} /></span><span><strong>Appearance</strong><small>Choose how OpnLauncher looks on this device.</small></span></div>
          <div className="theme-options" role="group" aria-label="Color theme">
            <button className={`theme-option ${theme === "dark" ? "selected" : ""}`} type="button" onClick={() => onThemeChange("dark")} aria-pressed={theme === "dark"}><span className="theme-option-icon dark-preview"><Moon size={18} /></span><span><strong>Midnight</strong><small>Dark theme</small></span>{theme === "dark" && <Check size={17} />}</button>
            <button className={`theme-option ${theme === "light" ? "selected" : ""}`} type="button" onClick={() => onThemeChange("light")} aria-pressed={theme === "light"}><span className="theme-option-icon light-preview"><Sun size={18} /></span><span><strong>Daylight</strong><small>Light theme</small></span>{theme === "light" && <Check size={17} />}</button>
          </div>
          <div className="accent-picker"><div><strong>Accent color</strong><small>Used for highlights and buttons.</small></div><div className="accent-swatches" role="group" aria-label="Accent color">{accentColors.map((color) => <button className={`accent-swatch swatch-${color.id} ${accent === color.id ? "selected" : ""}`} type="button" key={color.id} onClick={() => onAccentChange(color.id)} aria-label={`${color.label} accent`} aria-pressed={accent === color.id}><span />{accent === color.id && <Check size={13} />}</button>)}</div></div>
        </section>

        <section className="settings-card">
          <div className="settings-card-heading"><span className="settings-icon java-settings-icon"><Cpu size={18} /></span><span><strong>Java runtimes</strong><small>{runtimes.length} discovered on this device</small></span><span className="settings-heading-trailing">{runtimes.length}</span></div>
          {runtimes.length > 0 ? <div className="settings-runtime-list">{runtimes.map((runtime) => <div className="settings-runtime-row" key={runtime.path}><span className="runtime-check"><Check size={14} /></span><span className="settings-runtime-copy"><strong>{javaLabel(runtime.majorVersion)} · {runtime.vendor ?? "Java"}</strong><small title={runtime.path}>{runtime.path}</small></span><span className="runtime-arch">{runtime.architecture ?? "Unknown"}</span></div>)}</div> : <div className="settings-empty"><Cpu size={18} /><span>No Java runtime found. Set JAVA_HOME or install Java with your system package manager.</span></div>}
        </section>

        <section className="settings-card">
          <div className="settings-card-heading"><span className="settings-icon folder-settings-icon"><HardDrive size={18} /></span><span><strong>Minecraft locations</strong><small>Existing folders detected on this device.</small></span><button className="text-link settings-heading-action" type="button" onClick={refreshInstallations}>Rescan <ArrowRight size={14} /></button></div>
          {installations.length > 0 ? <div className="settings-location-list">{installations.map((installation) => <div className="settings-location-row" key={installation.root}><FolderOpen size={17} /><span><strong>{installation.versions.length} installed version{installation.versions.length === 1 ? "" : "s"}</strong><small title={installation.root}>{installation.root}</small></span><span className="location-count">{installation.versions.length}</span></div>)}</div> : <div className="settings-empty"><FolderOpen size={18} /><span>No Minecraft folders found yet. Add an installation from your library.</span></div>}
        </section>

        <section className="settings-card privacy-settings-card">
          <div className="settings-card-heading"><span className="settings-icon privacy-settings-icon"><ShieldCheck size={18} /></span><span><strong>Privacy by design</strong><small>No tracking, ads, telemetry, or launcher account.</small></span></div>
          <p>Theme, accent color, favorites, and selected instances and local profiles are saved on this device. When you browse or install Minecraft versions, OpnLauncher contacts Mojang’s documented metadata and file hosts directly.</p>
          <div className="privacy-settings-footer"><span><span className="status-pip" /> No background activity</span><span>See PRIVACY.md for network details.</span></div>
        </section>

        <section className="settings-card updates-card">
          <div className="settings-card-heading"><span className="settings-icon update-settings-icon"><ArrowDownToLine size={18} /></span><span><strong>Application updates</strong><small>Manual updates only</small></span></div>
          <p>Automatic update checks are not configured. OpnLauncher will not install updates or contact a launcher update server in the background.</p>
          <span className="update-status"><span /> Update checks are off</span>
        </section>
      </div>
    </>
  );
}

export default App;
