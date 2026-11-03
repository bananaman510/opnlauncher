import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AlertCircle, ArrowDownToLine, ChevronDown, LoaderCircle, RefreshCw, ShieldCheck, Wrench } from "lucide-react";
import type { InstalledLoaderProfile, Instance, JavaRuntime, LoaderVersion, ModInstanceContext, RemoteMinecraftVersion } from "../types";

type SetupLoader = "vanilla" | "fabric" | "forge";

export interface InstanceSetupSelection {
  instanceId: string;
  minecraftVersion: string;
  loader: SetupLoader;
  loaderVersion: string | null;
  javaPath: string | null;
}

interface InstanceSetupPanelProps {
  instance: Instance;
  versions: RemoteMinecraftVersion[];
  versionsLoading: boolean;
  versionsError: string;
  onRefreshVersions: () => void;
  runtimes: JavaRuntime[];
  selectedJavaPath: string;
  desktop: boolean;
  installing: boolean;
  onInstall: (selection: InstanceSetupSelection) => Promise<void>;
}

export function InstanceSetupPanel(props: InstanceSetupPanelProps) {
  const [minecraftVersion, setMinecraftVersion] = useState("");
  const [loader, setLoader] = useState<SetupLoader>("vanilla");
  const [loaderVersions, setLoaderVersions] = useState<LoaderVersion[]>([]);
  const [loaderVersion, setLoaderVersion] = useState("");
  const [currentLoaderFamily, setCurrentLoaderFamily] = useState("");
  const [currentLoaderVersion, setCurrentLoaderVersion] = useState("");
  const [javaPath, setJavaPath] = useState("");
  const [versionSearch, setVersionSearch] = useState("");
  const [contextLoading, setContextLoading] = useState(false);
  const [sharedGameDirectory, setSharedGameDirectory] = useState(false);
  const [loaderVersionsLoading, setLoaderVersionsLoading] = useState(false);
  const [setupError, setSetupError] = useState("");
  const [setupNote, setSetupNote] = useState("");
  const savedJavaPathMissing = Boolean(javaPath) && !props.runtimes.some((runtime) => runtime.path === javaPath);
  const requestedVersionList = useRef(false);

  useEffect(() => {
    setJavaPath(props.selectedJavaPath || props.runtimes[0]?.path || "");
  }, [props.instance.id, props.runtimes, props.selectedJavaPath]);

  useEffect(() => {
    if (!props.desktop) return;
    let cancelled = false;
    setContextLoading(true);
    Promise.all([
      invoke<ModInstanceContext>("inspect_instance_mod_context", { instanceId: props.instance.id }),
      invoke<InstalledLoaderProfile[]>("list_installed_loader_profiles", { instanceId: props.instance.id }),
    ])
      .then(([context, profiles]) => {
        if (cancelled) return;
        setMinecraftVersion(context.minecraftVersion ?? "");
        setLoader(context.loader === "fabric" || context.loader === "forge" ? context.loader : "vanilla");
        const activeProfile = profiles.find((profile) => profile.versionId === context.versionId);
        setCurrentLoaderFamily(activeProfile?.loader ?? "");
        setCurrentLoaderVersion(activeProfile?.loaderVersion ?? "");
        setSharedGameDirectory(context.sharedGameDirectory);
      })
      .catch((reason: unknown) => { if (!cancelled) setSetupError(String(reason)); })
      .finally(() => { if (!cancelled) setContextLoading(false); });
    return () => { cancelled = true; };
  }, [props.desktop, props.instance.id, props.instance.versionId]);

  useEffect(() => {
    if (!props.desktop || requestedVersionList.current || props.versions.length || props.versionsLoading) return;
    requestedVersionList.current = true;
    props.onRefreshVersions();
  }, [props.desktop, props.versions.length, props.versionsLoading, props.onRefreshVersions]);

  useEffect(() => {
    if (!minecraftVersion && props.versions.length) {
      setMinecraftVersion(props.versions.find((version) => version.kind === "release")?.id ?? props.versions[0].id);
    }
  }, [minecraftVersion, props.versions]);

  useEffect(() => {
    if (!props.desktop || loader === "vanilla" || !minecraftVersion) {
      setLoaderVersions([]);
      setLoaderVersion("");
      return;
    }
    let cancelled = false;
    setLoaderVersionsLoading(true);
    setSetupError("");
    invoke<LoaderVersion[]>("list_loader_versions", { loader, gameVersion: minecraftVersion })
      .then((available) => {
        if (cancelled) return;
        setLoaderVersions(available);
        setLoaderVersion((current) => available.some((entry) => entry.version === current)
          ? current
          : currentLoaderFamily === loader && available.some((entry) => entry.version === currentLoaderVersion)
            ? currentLoaderVersion
            : available[0]?.version ?? "");
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setLoaderVersions([]);
          setLoaderVersion("");
          setSetupError(String(reason));
        }
      })
      .finally(() => { if (!cancelled) setLoaderVersionsLoading(false); });
    return () => { cancelled = true; };
  }, [currentLoaderFamily, currentLoaderVersion, loader, minecraftVersion, props.desktop]);

  const matchingVersions = useMemo(() => {
    const query = versionSearch.trim().toLocaleLowerCase();
    if (!query) return props.versions;
    return props.versions.filter((version) => `${version.id} ${version.kind}`.toLocaleLowerCase().includes(query));
  }, [props.versions, versionSearch]);

  async function installSelectedSetup() {
    if (!minecraftVersion || (loader !== "vanilla" && !loaderVersion)) return;
    setSetupError("");
    setSetupNote("");
    try {
      await props.onInstall({
        instanceId: props.instance.id,
        minecraftVersion,
        loader,
        loaderVersion: loader === "vanilla" ? null : loaderVersion,
        javaPath: loader === "forge" ? javaPath || null : null,
      });
      setSetupNote(`${minecraftVersion}${loader === "vanilla" ? "" : ` · ${loader === "fabric" ? "Fabric" : "Forge"} ${loaderVersion}`} is ready in this instance.`);
    } catch (reason) {
      setSetupError(String(reason));
    }
  }

  const selectedGameVersionListed = props.versions.some((version) => version.id === minecraftVersion);
  const selectedJavaRuntimeAvailable = props.runtimes.some((runtime) => runtime.path === javaPath);
  const currentLoaderLabel = loader === "fabric" ? "Fabric" : loader === "forge" ? "Forge" : "Vanilla";

  return (
    <section className="instance-setup-panel" aria-labelledby="instance-setup-title">
      <div className="instance-setup-heading">
        <span className="instance-setup-icon"><Wrench size={16} /></span>
        <span><strong id="instance-setup-title">Game setup</strong><small>Choose this instance’s Minecraft version and loader.</small></span>
        {props.versionsLoading || contextLoading ? <LoaderCircle className="spin" size={15} /> : null}
      </div>
      <p className="instance-setup-copy">Downloads go into this instance’s game folder. Existing saves and mods are kept, and files already present are verified before reuse.{sharedGameDirectory ? " This folder is shared with another instance, so its game files are shared too." : ""}</p>
      <div className="instance-setup-fields">
        <label className="instance-setup-field"><span>Minecraft version</span><input className="field-control instance-version-search" aria-label="Search Minecraft versions" placeholder="Search versions" value={versionSearch} onChange={(event) => setVersionSearch(event.target.value)} disabled={props.versionsLoading} /><span className="select-wrap"><select className="field-control" aria-label="Minecraft version" value={minecraftVersion} onChange={(event) => setMinecraftVersion(event.target.value)} disabled={!props.versions.length || props.versionsLoading}>{minecraftVersion && !selectedGameVersionListed && <option value={minecraftVersion}>Current · {minecraftVersion}</option>}{minecraftVersion && selectedGameVersionListed && !matchingVersions.some((version) => version.id === minecraftVersion) && <option value={minecraftVersion}>Selected · {minecraftVersion}</option>}{matchingVersions.map((version) => <option key={version.id} value={version.id}>{version.id} · {version.kind.replace("_", " ")}</option>)}</select><ChevronDown size={15} /></span></label>
        <label className="instance-setup-field"><span>Mod loader</span><span className="select-wrap"><select className="field-control" aria-label="Mod loader" value={loader} onChange={(event) => setLoader(event.target.value as SetupLoader)}><option value="vanilla">Vanilla</option><option value="fabric">Fabric</option><option value="forge">Forge</option></select><ChevronDown size={15} /></span></label>
        {loader !== "vanilla" && <label className="instance-setup-field"><span>{currentLoaderLabel} version</span><span className="select-wrap"><select className="field-control" aria-label={`${currentLoaderLabel} version`} value={loaderVersion} onChange={(event) => setLoaderVersion(event.target.value)} disabled={loaderVersionsLoading || !loaderVersions.length}><option value="">{loaderVersionsLoading ? "Loading compatible builds…" : loaderVersions.length ? "Choose a build" : "No compatible build found"}</option>{loaderVersions.map((version) => <option key={version.version} value={version.version}>{version.version}{version.recommended ? " · Recommended" : version.stable ? " · Stable" : " · Preview"}</option>)}</select><ChevronDown size={15} /></span></label>}
        {loader === "forge" && <label className="instance-setup-field"><span>Java for Forge installer</span><span className="select-wrap"><select className="field-control" aria-label="Java for Forge installer" value={javaPath} onChange={(event) => setJavaPath(event.target.value)}><option value="">Choose Java runtime</option>{savedJavaPathMissing && <option value={javaPath}>Saved Java path unavailable</option>}{props.runtimes.map((runtime) => <option key={runtime.path} value={runtime.path}>{`Java ${runtime.majorVersion} · ${runtime.vendor ?? "Runtime"}`}</option>)}</select><ChevronDown size={15} /></span>{!props.runtimes.length && <small>Add or discover a Java runtime in Settings → Java Manager before installing Forge.</small>}</label>}
      </div>
      {props.versionsError && <p className="instance-setup-error" role="alert"><AlertCircle size={14} />{props.versionsError}<button className="text-link" type="button" onClick={props.onRefreshVersions}>Retry</button></p>}
      {setupError && <p className="instance-setup-error" role="alert"><AlertCircle size={14} />{setupError}</p>}
      {setupNote && <p className="instance-setup-success"><ShieldCheck size={14} />{setupNote}</p>}
      <div className="instance-setup-actions">
        <button className="secondary-button" type="button" onClick={props.onRefreshVersions} disabled={!props.desktop || props.versionsLoading}><RefreshCw size={14} /> Refresh versions</button>
        <button className="play-button" type="button" onClick={() => void installSelectedSetup()} disabled={!props.desktop || props.installing || props.versionsLoading || !minecraftVersion || !selectedGameVersionListed || (loader !== "vanilla" && (!loaderVersion || loaderVersionsLoading)) || (loader === "forge" && !selectedJavaRuntimeAvailable)}>
          {props.installing ? <LoaderCircle className="spin" size={15} /> : <ArrowDownToLine size={15} />}{props.installing ? "Installing setup…" : `Install ${loader === "vanilla" ? "Minecraft" : `${currentLoaderLabel} setup`}`}
        </button>
      </div>
      <small className="instance-setup-integrity">Loader builds are matched to {minecraftVersion || "the selected Minecraft version"}; Modrinth and CurseForge results will follow this instance automatically.</small>
    </section>
  );
}
