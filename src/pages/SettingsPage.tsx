import { useEffect, useMemo, useState } from "react";
import type { FormEvent, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowRight,
  Check,
  ChevronDown,
  CircleAlert,
  CircleCheck,
  Cpu,
  Download,
  FolderOpen,
  HardDrive,
  Info,
  LoaderCircle,
  Moon,
  Palette,
  Plus,
  RefreshCw,
  Settings2,
  ShieldCheck,
  Sun,
  Wrench,
} from "lucide-react";
import { javaLabel } from "../format";
import type { Instance, InstalledLoaderProfile, JavaRuntime, LoaderVersion, MinecraftInstallation } from "../types";

type ThemeMode = "dark" | "light";
type AccentColor = "lime" | "mint" | "sky" | "violet";
type SettingsSection = "general" | "appearance" | "java" | "loaders" | "storage" | "privacy" | "about";
type LoaderFamily = "fabric" | "forge";

const accentColors: { id: AccentColor; label: string }[] = [
  { id: "violet", label: "Violet" },
  { id: "lime", label: "Citrus" },
  { id: "mint", label: "Mint" },
  { id: "sky", label: "Sky" },
];

const sections: { id: SettingsSection; label: string; icon: typeof Settings2 }[] = [
  { id: "general", label: "General", icon: Settings2 },
  { id: "appearance", label: "Appearance", icon: Palette },
  { id: "java", label: "Java manager", icon: Cpu },
  { id: "loaders", label: "Mod loaders", icon: Wrench },
  { id: "storage", label: "Storage", icon: HardDrive },
  { id: "privacy", label: "Privacy", icon: ShieldCheck },
  { id: "about", label: "About", icon: Info },
];

interface SettingsPageProps {
  runtimes: JavaRuntime[];
  installations: MinecraftInstallation[];
  instances: Instance[];
  selectedInstance: Instance | null;
  refreshInstallations: () => Promise<void>;
  refreshRuntimes: () => Promise<void>;
  addRuntime: (path: string) => Promise<void>;
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
  accent: AccentColor;
  onAccentChange: (accent: AccentColor) => void;
  defaultJavaPath: string;
  onDefaultJavaChange: (path: string) => void;
  defaultMemoryMb: number;
  onDefaultMemoryChange: (memory: number) => void;
  desktop: boolean;
  appVersion: string;
  onInstanceUpdated: (instance: Instance) => void;
  onBrowseVersions: () => void;
  onOpenInstances: () => void;
}

export function SettingsPage(props: SettingsPageProps) {
  const [section, setSection] = useState<SettingsSection>("general");
  const [javaPathInput, setJavaPathInput] = useState("");
  const [javaPathError, setJavaPathError] = useState("");

  async function addJavaPath(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const path = javaPathInput.trim();
    if (!path) return;
    setJavaPathError("");
    try {
      await props.addRuntime(path);
      setJavaPathInput("");
    } catch (reason) {
      setJavaPathError(String(reason));
    }
  }

  return (
    <>
      <section className="page-heading settings-page-heading">
        <div><p className="eyebrow"><Settings2 size={14} /> PERSONALIZE YOUR LAUNCHER</p><h1>Settings</h1><p className="page-subtitle">Control your game setup, runtimes, storage, and privacy.</p></div>
      </section>
      <div className="settings-workspace">
        <aside className="settings-nav" aria-label="Settings sections">
          <strong className="settings-nav-title">Settings</strong>
          <nav>{sections.map(({ id, label, icon: Icon }) => <button key={id} type="button" className={section === id ? "selected" : ""} onClick={() => setSection(id)} aria-current={section === id ? "page" : undefined}><Icon size={16} /><span>{label}</span>{section === id && <span className="settings-nav-indicator" />}</button>)}</nav>
          <div className="settings-nav-note"><ShieldCheck size={15} /><span>Private by default</span></div>
        </aside>
        <main className="settings-content">
          {section === "general" && <GeneralSettings {...props} />}
          {section === "appearance" && <AppearanceSettings theme={props.theme} onThemeChange={props.onThemeChange} accent={props.accent} onAccentChange={props.onAccentChange} />}
          {section === "java" && <JavaSettings {...props} javaPathInput={javaPathInput} onJavaPathChange={setJavaPathInput} onAddJavaPath={addJavaPath} javaPathError={javaPathError} />}
          {section === "loaders" && <LoaderSettings {...props} />}
          {section === "storage" && <StorageSettings {...props} />}
          {section === "privacy" && <PrivacySettings />}
          {section === "about" && <AboutSettings version={props.appVersion} />}
        </main>
      </div>
    </>
  );
}

function GeneralSettings({
  runtimes,
  defaultJavaPath,
  onDefaultJavaChange,
  defaultMemoryMb,
  onDefaultMemoryChange,
}: SettingsPageProps) {
  const savedRuntimeMissing = defaultJavaPath && !runtimes.some((runtime) => runtime.path === defaultJavaPath);
  return (
    <SettingsSection title="General" subtitle="Defaults used when you create a new instance.">
      <SettingsGroup title="New instances" description="These defaults can be changed later for each instance.">
        <div className="settings-control-row">
          <span><strong>Default Java runtime</strong><small>Use a device runtime or let OpnLauncher choose.</small></span>
          <div className="settings-select-wrap"><select aria-label="Default Java runtime" value={defaultJavaPath} onChange={(event) => onDefaultJavaChange(event.target.value)}><option value="">Automatic selection</option>{savedRuntimeMissing && <option value={defaultJavaPath}>Saved Java path unavailable</option>}{runtimes.map((runtime) => <option key={runtime.path} value={runtime.path}>{javaLabel(runtime.majorVersion)} · {runtime.vendor ?? "Java"}</option>)}</select><ChevronDown size={15} /></div>
        </div>
        <div className="settings-control-row memory-default-row">
          <span><strong>Default memory allocation</strong><small>Maximum RAM for newly created instances.</small></span>
          <div className="default-memory-control"><input aria-label="Default memory in gigabytes" type="range" min="1024" max="16384" step="512" value={defaultMemoryMb} onChange={(event) => onDefaultMemoryChange(Number(event.target.value))} /><strong>{formatMemory(defaultMemoryMb)}</strong></div>
        </div>
      </SettingsGroup>
      <div className="settings-callout"><Info size={16} /><span>Java is selected per instance at launch. These defaults only prefill new instances and never change an existing instance automatically.</span></div>
    </SettingsSection>
  );
}

function AppearanceSettings({ theme, onThemeChange, accent, onAccentChange }: Pick<SettingsPageProps, "theme" | "onThemeChange" | "accent" | "onAccentChange">) {
  return (
    <SettingsSection title="Appearance" subtitle="Choose a comfortable look for OpnLauncher on this device.">
      <SettingsGroup title="Color theme" description="The dark theme is easier on the eyes in low light.">
        <div className="theme-options" role="group" aria-label="Color theme">
          <button className={`theme-option ${theme === "dark" ? "selected" : ""}`} type="button" onClick={() => onThemeChange("dark")} aria-pressed={theme === "dark"}><span className="theme-option-icon dark-preview"><Moon size={18} /></span><span><strong>Midnight</strong><small>Dark theme</small></span>{theme === "dark" && <Check size={17} />}</button>
          <button className={`theme-option ${theme === "light" ? "selected" : ""}`} type="button" onClick={() => onThemeChange("light")} aria-pressed={theme === "light"}><span className="theme-option-icon light-preview"><Sun size={18} /></span><span><strong>Daylight</strong><small>Light theme</small></span>{theme === "light" && <Check size={17} />}</button>
        </div>
      </SettingsGroup>
      <SettingsGroup title="Accent color" description="Used for active navigation, buttons, and selection highlights.">
        <div className="accent-choice-list" role="group" aria-label="Accent color">{accentColors.map((color) => <button key={color.id} className={`accent-choice ${accent === color.id ? "selected" : ""}`} type="button" onClick={() => onAccentChange(color.id)} aria-pressed={accent === color.id}><span className={`accent-choice-dot swatch-${color.id}`} /><strong>{color.label}</strong>{accent === color.id && <Check size={15} />}</button>)}</div>
      </SettingsGroup>
    </SettingsSection>
  );
}

function JavaSettings({
  runtimes,
  defaultJavaPath,
  onDefaultJavaChange,
  refreshRuntimes,
  desktop,
  javaPathInput,
  onJavaPathChange,
  onAddJavaPath,
  javaPathError,
}: SettingsPageProps & { javaPathInput: string; onJavaPathChange: (value: string) => void; onAddJavaPath: (event: FormEvent<HTMLFormElement>) => void; javaPathError: string }) {
  const savedRuntimeMissing = defaultJavaPath && !runtimes.some((runtime) => runtime.path === defaultJavaPath);
  return (
    <SettingsSection title="Java manager" subtitle="Choose the Java runtime used by new and existing instances.">
      <SettingsGroup title="Preferred runtime" description="Minecraft may need different Java versions. You can override this on an instance.">
        <div className="settings-control-row">
          <span><strong>Default runtime</strong><small>Applied to new instances.</small></span>
          <div className="settings-select-wrap"><select aria-label="Preferred Java runtime" value={defaultJavaPath} onChange={(event) => onDefaultJavaChange(event.target.value)}><option value="">Automatic selection</option>{savedRuntimeMissing && <option value={defaultJavaPath}>Saved Java path unavailable</option>}{runtimes.map((runtime) => <option key={runtime.path} value={runtime.path}>{javaLabel(runtime.majorVersion)} · {runtime.vendor ?? "Java"}</option>)}</select><ChevronDown size={15} /></div>
        </div>
      </SettingsGroup>
      <SettingsGroup title="Detected runtimes" description={`${runtimes.length} Java runtime${runtimes.length === 1 ? "" : "s"} available on this device.`} action={<button className="settings-quiet-button" type="button" onClick={() => void refreshRuntimes()} disabled={!desktop}><RefreshCw size={14} /> Rescan</button>}>
        {runtimes.length ? <div className="java-manager-list">{runtimes.map((runtime) => <article className="java-manager-card" key={runtime.path}>
          <span className="java-manager-icon"><Cpu size={18} /></span>
          <div className="java-manager-main"><strong>{javaLabel(runtime.majorVersion)} <span>{runtime.version}</span></strong><small title={runtime.path}>{runtime.path}</small><div className="java-manager-tags"><span>{runtime.vendor ?? "Java runtime"}</span><span>{runtime.architecture ?? "Architecture unknown"}</span></div></div>
          {defaultJavaPath === runtime.path ? <span className="runtime-default-badge"><Check size={13} /> Default</span> : <button className="settings-quiet-button" type="button" onClick={() => onDefaultJavaChange(runtime.path)}>Set default</button>}
        </article>)}</div> : <div className="settings-empty"><Cpu size={18} /><span>No Java runtime found. Install Java with your system package manager, then rescan.</span></div>}
      </SettingsGroup>
      <SettingsGroup title="Add a Java executable" description="Use a runtime outside the standard system search paths.">
        <form className="java-path-form" onSubmit={onAddJavaPath}><input className="field-control" aria-label="Java executable path" placeholder="/path/to/java or java.exe" value={javaPathInput} onChange={(event) => onJavaPathChange(event.target.value)} /><button className="secondary-button" type="submit" disabled={!desktop}><Plus size={15} /> Add runtime</button></form>
        {javaPathError && <p className="settings-inline-error" role="alert"><CircleAlert size={14} /> {javaPathError}</p>}
      </SettingsGroup>
      <div className="settings-callout"><CircleCheck size={16} /><span>Java versions are checked locally before they can be saved. OpnLauncher does not install or modify system runtimes.</span></div>
    </SettingsSection>
  );
}

function LoaderSettings(props: SettingsPageProps) {
  const [family, setFamily] = useState<LoaderFamily>("fabric");
  const [minecraftVersion, setMinecraftVersion] = useState("");
  const [profiles, setProfiles] = useState<InstalledLoaderProfile[]>([]);
  const [versions, setVersions] = useState<LoaderVersion[]>([]);
  const [selectedVersion, setSelectedVersion] = useState("");
  const [profilesLoading, setProfilesLoading] = useState(false);
  const [versionsLoading, setVersionsLoading] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [loaderError, setLoaderError] = useState("");
  const [javaPath, setJavaPath] = useState(props.defaultJavaPath);
  const savedJavaPathMissing = javaPath && !props.runtimes.some((runtime) => runtime.path === javaPath);

  useEffect(() => {
    setJavaPath(props.selectedInstance?.javaPath || props.defaultJavaPath || props.runtimes[0]?.path || "");
  }, [props.defaultJavaPath, props.runtimes, props.selectedInstance?.id, props.selectedInstance?.javaPath]);

  useEffect(() => {
    if (!props.desktop || !props.selectedInstance || !props.selectedInstance.versionId) {
      setProfiles([]);
      setMinecraftVersion("");
      return;
    }
    let cancelled = false;
    setProfilesLoading(true);
    setLoaderError("");
    Promise.all([
      invoke<InstalledLoaderProfile[]>("list_installed_loader_profiles", { instanceId: props.selectedInstance.id }),
      invoke<{ minecraftVersion: string | null; loader: string | null }>("inspect_instance_mod_context", { instanceId: props.selectedInstance.id }),
    ]).then(([installed, context]) => {
      if (cancelled) return;
      setProfiles(installed);
      const gameVersion = context.minecraftVersion ?? props.selectedInstance?.versionId ?? "";
      setMinecraftVersion(gameVersion);
      if (context.loader === "forge" || context.loader === "fabric") setFamily(context.loader);
    }).catch((reason: unknown) => {
      if (!cancelled) setLoaderError(String(reason));
    }).finally(() => {
      if (!cancelled) setProfilesLoading(false);
    });
    return () => { cancelled = true; };
  }, [props.desktop, props.selectedInstance?.id, props.selectedInstance?.versionId]);

  useEffect(() => {
    if (!props.desktop || !minecraftVersion) {
      setVersions([]);
      setSelectedVersion("");
      return;
    }
    let cancelled = false;
    setVersionsLoading(true);
    setLoaderError("");
    invoke<LoaderVersion[]>("list_loader_versions", { loader: family, gameVersion: minecraftVersion })
      .then((available) => {
        if (cancelled) return;
        setVersions(available);
        setSelectedVersion((current) => available.some((entry) => entry.version === current) ? current : available[0]?.version ?? "");
      })
      .catch((reason: unknown) => { if (!cancelled) { setVersions([]); setLoaderError(String(reason)); } })
      .finally(() => { if (!cancelled) setVersionsLoading(false); });
    return () => { cancelled = true; };
  }, [family, minecraftVersion, props.desktop]);

  const visibleProfiles = useMemo(() => profiles.filter((profile) => profile.minecraftVersion === minecraftVersion), [minecraftVersion, profiles]);
  const loaderVersionsInstalled = visibleProfiles.length;

  async function refreshProfiles() {
    if (!props.selectedInstance) return;
    const installed = await invoke<InstalledLoaderProfile[]>("list_installed_loader_profiles", { instanceId: props.selectedInstance.id });
    setProfiles(installed);
  }

  async function installLoader() {
    if (!props.desktop || !props.selectedInstance || !selectedVersion) return;
    setInstalling(true);
    setLoaderError("");
    try {
      const updated = family === "fabric"
        ? await invoke<Instance>("install_fabric_loader", { instanceId: props.selectedInstance.id, loaderVersion: selectedVersion })
        : await invoke<Instance>("install_forge_loader", { instanceId: props.selectedInstance.id, loaderVersion: selectedVersion, javaPath });
      props.onInstanceUpdated(updated);
      await refreshProfiles();
    } catch (reason) {
      setLoaderError(String(reason));
    } finally {
      setInstalling(false);
    }
  }

  async function useInstalledVersion(versionId: string) {
    if (!props.selectedInstance) return;
    setLoaderError("");
    try {
      const updated = await invoke<Instance>("set_instance_version", { instanceId: props.selectedInstance.id, versionId });
      props.onInstanceUpdated(updated);
    } catch (reason) {
      setLoaderError(String(reason));
    }
  }

  return (
    <SettingsSection title="Mod loaders" subtitle="Install a compatible loader build or switch to one already in this game folder.">
      <div className="loader-instance-strip">
        <span className="loader-instance-icon"><Wrench size={18} /></span>
        <span><strong>{props.selectedInstance?.name ?? "Select an instance first"}</strong><small>{minecraftVersion ? `Minecraft ${minecraftVersion}` : "Install a Minecraft version before adding a loader."}</small></span>
        <button type="button" className="settings-quiet-button" onClick={props.onOpenInstances}>Choose instance <ArrowRight size={14} /></button>
      </div>
      {props.selectedInstance && !props.selectedInstance.versionId && <div className="settings-callout"><Info size={16} /><span>Choose a Minecraft version first, then return here to install Fabric or Forge.</span><button className="text-link" type="button" onClick={props.onBrowseVersions}>Browse versions <ArrowRight size={14} /></button></div>}
      <SettingsGroup title="Install a loader" description="Loader files are added to the selected game folder. Existing worlds and mods are kept.">
        <div className="loader-tabs" role="tablist" aria-label="Loader family">
          <button type="button" role="tab" aria-selected={family === "fabric"} className={family === "fabric" ? "selected" : ""} onClick={() => setFamily("fabric")}><span className="loader-tab-dot fabric-dot" />Fabric</button>
          <button type="button" role="tab" aria-selected={family === "forge"} className={family === "forge" ? "selected" : ""} onClick={() => setFamily("forge")}><span className="loader-tab-dot forge-dot" />Forge</button>
        </div>
        <div className="loader-install-panel">
          <div className="loader-install-copy"><strong>{family === "fabric" ? "Fabric Loader" : "Minecraft Forge"}</strong><small>{family === "fabric" ? "Lightweight loader with direct profile metadata and verified library downloads." : "Forge’s official client installer, checksum verified before it runs for this instance."}</small></div>
          <div className="loader-install-controls">
            {family === "forge" && <div className="settings-select-wrap"><select aria-label="Java for Forge installer" value={javaPath} onChange={(event) => setJavaPath(event.target.value)}><option value="">Choose Java runtime</option>{savedJavaPathMissing && <option value={javaPath}>Saved Java path unavailable</option>}{props.runtimes.map((runtime) => <option value={runtime.path} key={runtime.path}>{javaLabel(runtime.majorVersion)} · {runtime.vendor ?? "Java"}</option>)}</select><ChevronDown size={15} /></div>}
            <div className="settings-select-wrap"><select aria-label={`${family === "fabric" ? "Fabric" : "Forge"} version`} value={selectedVersion} onChange={(event) => setSelectedVersion(event.target.value)} disabled={!versions.length || versionsLoading}><option value="">{versionsLoading ? "Loading compatible builds…" : versions.length ? "Choose a loader build" : "No build available"}</option>{versions.map((version) => <option key={version.version} value={version.version}>{version.version}{version.recommended ? " · Recommended" : version.stable ? " · Stable" : " · Preview"}</option>)}</select><ChevronDown size={15} /></div>
            <button className="play-button" type="button" onClick={() => void installLoader()} disabled={!props.desktop || !props.selectedInstance?.versionId || !selectedVersion || installing || (family === "forge" && !javaPath)}>{installing ? <LoaderCircle size={15} className="spin" /> : <Download size={15} />}{installing ? "Installing…" : `Install ${family === "fabric" ? "Fabric" : "Forge"}`}</button>
          </div>
        </div>
        {family === "forge" && <div className="settings-callout forge-callout"><ShieldCheck size={15} /><span>The selected Forge installer is downloaded from Forge’s Maven repository and checked against its published SHA-1 before it runs. Forge installer network activity is handled by that official installer.</span></div>}
      </SettingsGroup>
      <SettingsGroup title="Installed loader profiles" description={profilesLoading ? "Scanning this game folder…" : `${loaderVersionsInstalled} compatible loader profile${loaderVersionsInstalled === 1 ? "" : "s"} found for this Minecraft version.`} action={<button className="settings-quiet-button" type="button" onClick={() => void refreshProfiles()} disabled={!props.selectedInstance || profilesLoading}><RefreshCw size={14} /> Rescan</button>}>
        {visibleProfiles.length ? <div className="installed-loader-list">{visibleProfiles.map((profile) => {
          const selected = props.selectedInstance?.versionId === profile.versionId;
          return <article className={`installed-loader-row ${selected ? "selected" : ""}`} key={profile.versionId}><span className={`installed-loader-mark ${profile.loader}`}><Wrench size={15} /></span><span className="installed-loader-copy"><strong>{profile.loader === "neoforge" ? "NeoForge" : profile.loader[0].toUpperCase() + profile.loader.slice(1)} <span>{profile.loaderVersion}</span></strong><small>{profile.versionId}</small></span>{selected ? <span className="runtime-default-badge"><Check size={13} /> Active</span> : <button className="secondary-button" type="button" onClick={() => void useInstalledVersion(profile.versionId)}>Use this version</button>}</article>;
        })}</div> : <div className="settings-empty"><Wrench size={18} /><span>{props.selectedInstance?.versionId ? "No loader profiles for this Minecraft version are installed in the selected game folder yet." : "Select an instance with a Minecraft version first."}</span></div>}
      </SettingsGroup>
      {loaderError && <div className="error-banner loader-error" role="alert"><CircleAlert size={16} /><span>{loaderError}</span></div>}
      {!props.desktop && <div className="settings-callout"><Info size={16} /><span>Open the desktop build to browse loader metadata and install game files.</span><button className="text-link" type="button" onClick={props.onBrowseVersions}>Browse Minecraft versions <ArrowRight size={14} /></button></div>}
      <div className="settings-callout"><ShieldCheck size={16} /><span>Installing or switching a loader leaves the instance’s saves and mods in place. Mods still contain executable code; install them only from sources you trust.</span></div>
    </SettingsSection>
  );
}

function StorageSettings({ installations, instances, refreshInstallations }: SettingsPageProps) {
  return (
    <SettingsSection title="Storage" subtitle="Review Minecraft folders and the separate directories used by your instances.">
      <SettingsGroup title="Detected Minecraft locations" description="Rescanning only reads local version metadata." action={<button className="settings-quiet-button" type="button" onClick={() => void refreshInstallations()}><RefreshCw size={14} /> Rescan</button>}>
        {installations.length ? <div className="storage-location-list">{installations.map((installation) => <article className="storage-location-card" key={installation.root}><span className="storage-icon"><FolderOpen size={17} /></span><span><strong>{installation.versions.length} installed version{installation.versions.length === 1 ? "" : "s"}</strong><small title={installation.root}>{installation.root}</small></span><span className="storage-size">{installation.versions.length} versions</span></article>)}</div> : <div className="settings-empty"><FolderOpen size={18} /><span>No existing Minecraft locations were found. Use Instances → Add instance to inspect a custom folder.</span></div>}
      </SettingsGroup>
      <SettingsGroup title="Instance folders" description="Each managed version is stored separately to keep saves, mods, and settings isolated.">
        {instances.length ? <div className="storage-location-list">{instances.map((instance) => <article className="storage-location-card" key={instance.id}><span className="storage-icon instance-storage-icon"><HardDrive size={17} /></span><span><strong>{instance.name} <em>{instance.versionId ?? "No version"}</em></strong><small title={instance.gameDir}>{instance.gameDir}</small></span><span className="storage-size">Local</span></article>)}</div> : <div className="settings-empty"><HardDrive size={18} /><span>Your managed instances will appear here.</span></div>}
      </SettingsGroup>
      <div className="settings-callout"><Info size={16} /><span>Removing an instance from the library does not delete its game folder. Change or clean up local files from the instance itself.</span></div>
    </SettingsSection>
  );
}

function PrivacySettings() {
  return (
    <SettingsSection title="Privacy" subtitle="Network requests happen only when you use a feature that needs them.">
      <div className="privacy-feature-card"><span className="privacy-feature-icon"><ShieldCheck size={20} /></span><div><strong>No tracking or background reporting</strong><p>OpnLauncher does not include analytics, telemetry, advertising, crash reporting, or an OpnLauncher account.</p></div><span className="privacy-online-status"><i /> Idle</span></div>
      <SettingsGroup title="When you browse or install" description="Your device contacts the service needed for the action you started.">
        <div className="privacy-service-list"><div><span className="service-dot mojang-dot" /><span><strong>Mojang metadata and game files</strong><small>Version lists and selected Minecraft files are requested directly from Mojang’s documented hosts.</small></span></div><div><span className="service-dot fabric-dot" /><span><strong>Fabric or Forge metadata</strong><small>Loader version lists, profiles, and dependencies come from the corresponding official project hosts.</small></span></div><div><span className="service-dot modrinth-dot" /><span><strong>Modrinth and CurseForge</strong><small>Requests occur when you use the mod browser. CurseForge needs a user supplied API key.</small></span></div></div>
      </SettingsGroup>
      <div className="settings-callout"><ShieldCheck size={16} /><span>Settings, local profiles, favorites, and instance metadata stay on this device. Logs are local and are never uploaded automatically.</span></div>
    </SettingsSection>
  );
}

function AboutSettings({ version }: { version: string }) {
  return (
    <SettingsSection title="About" subtitle="Project information and current application build.">
      <div className="about-app-card"><span className="about-mark">OP</span><div><strong>OpnLauncher</strong><small>Independent, local-first Minecraft Java Edition launcher</small></div><span className="about-version">{version}</span></div>
      <SettingsGroup title="Project" description="Built with Tauri, Rust, React, TypeScript, and SQLite.">
        <div className="about-detail-row"><span>License</span><strong>GPL-3.0-or-later</strong></div>
        <div className="about-detail-row"><span>Developer profiles</span><strong>Available · clearly marked unverified</strong></div>
        <div className="about-detail-row"><span>Updates</span><strong>Manual only</strong></div>
      </SettingsGroup>
      <div className="settings-callout"><Info size={16} /><span>OpnLauncher is an independent community project and is not affiliated with, endorsed by, or associated with Mojang or Microsoft.</span></div>
    </SettingsSection>
  );
}

function SettingsSection({ title, subtitle, children }: { title: string; subtitle: string; children: ReactNode }) {
  return <section className="settings-section-view"><div className="settings-section-heading"><h2>{title}</h2><p>{subtitle}</p></div>{children}</section>;
}

function SettingsGroup({ title, description, action, children }: { title: string; description: string; action?: ReactNode; children: ReactNode }) {
  return <section className="settings-group"><header><span><strong>{title}</strong><small>{description}</small></span>{action}</header>{children}</section>;
}

function formatMemory(memoryMb: number): string {
  const gigabytes = memoryMb / 1024;
  return `${Number.isInteger(gigabytes) ? gigabytes : gigabytes.toFixed(1)} GB`;
}
