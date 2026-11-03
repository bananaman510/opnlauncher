import { ArrowDownToLine, CircleCheck, CircleX, LoaderCircle } from "lucide-react";
import type { DownloadProgress } from "../types";

interface DownloadsPageProps {
  progress: DownloadProgress | null;
  active: boolean;
  onCancel: () => void;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; value >= 1024 && index < units.length; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value.toFixed(1)} ${unit}`;
}

export function DownloadsPage({ progress, active, onCancel }: DownloadsPageProps) {
  const percent = progress
    ? progress.totalBytes > 0
      ? Math.min(100, Math.round((progress.downloadedBytes / progress.totalBytes) * 100))
      : progress.totalFiles > 0
        ? Math.round((progress.completedFiles / progress.totalFiles) * 100)
        : 0
    : 0;
  const complete = progress?.phase === "Installation complete";
  const cancelled = progress?.phase === "Installation cancelled";

  return (
    <>
      <section className="welcome-row subpage-title">
        <div><p className="eyebrow">LOCAL TRANSFERS</p><h1>Downloads</h1><p className="page-subtitle">Track version files installed from official Minecraft metadata.</p></div>
      </section>
      {progress ? (
        <section className="settings-panel download-panel">
          <div className="settings-panel-header">
            <div className="info-card-icon violet">{active ? <LoaderCircle className="spin" size={17} /> : complete ? <CircleCheck size={17} /> : cancelled ? <CircleX size={17} /> : <ArrowDownToLine size={17} />}</div>
            <div><strong>Minecraft {progress.versionId}</strong><span>{active ? progress.phase : complete ? "Installed successfully" : cancelled ? "Cancelled" : progress.phase}</span></div>
            {active && <button className="secondary-button" type="button" onClick={onCancel}>Cancel</button>}
          </div>
          <div className="download-progress-track" role="progressbar" aria-label={`Downloading Minecraft ${progress.versionId}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={percent}><span style={{ width: `${percent}%` }} /></div>
          <div className="download-progress-meta"><span>{percent}% · {progress.completedFiles} of {progress.totalFiles} files</span><span>{formatBytes(progress.downloadedBytes)}{progress.totalBytes > 0 ? ` / ${formatBytes(progress.totalBytes)}` : ""}</span></div>
          {active && progress.currentFile && <p className="download-current-file">{progress.currentFile}</p>}
        </section>
      ) : (
        <div className="empty-library large-empty"><div className="empty-library-icon"><ArrowDownToLine size={20} /></div><div><strong>No active downloads</strong><span>When you install a Minecraft version, its file progress will appear here.</span></div></div>
      )}
    </>
  );
}
