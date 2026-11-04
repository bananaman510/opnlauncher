import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChartColumnIncreasing, CircleAlert, Clock3, LoaderCircle, ShieldCheck } from "lucide-react";
import { aggregatePlaytime, createPlaytimeWindow, formatPlaytime, type PlaytimeRange } from "../playtime";
import type { PlaytimeSummary } from "../types";

const rangeOptions: { id: PlaytimeRange; label: string }[] = [
  { id: "7d", label: "7 days" },
  { id: "30d", label: "30 days" },
  { id: "90d", label: "90 days" },
  { id: "12m", label: "12 months" },
];

function readSavedRange(): PlaytimeRange {
  try {
    const stored = window.localStorage.getItem("opnlauncher.playtime-range");
    if (rangeOptions.some((option) => option.id === stored)) return stored as PlaytimeRange;
  } catch {
    // Keep the default range when local preferences are unavailable.
  }
  return "7d";
}

export function PlaytimeCard({ desktop }: { desktop: boolean }) {
  const [trackingEnabled, setTrackingEnabled] = useState(false);
  const [settingsLoaded, setSettingsLoaded] = useState(false);
  const [savingSetting, setSavingSetting] = useState(false);
  const [range, setRange] = useState<PlaytimeRange>(readSavedRange);
  const [summary, setSummary] = useState<PlaytimeSummary | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    if (!desktop) {
      setSettingsLoaded(true);
      return;
    }
    let cancelled = false;
    void invoke<boolean>("get_playtime_tracking_enabled").then((enabled) => {
      if (!cancelled) setTrackingEnabled(enabled);
    }).catch((reason) => {
      if (!cancelled) setError(String(reason));
    }).finally(() => {
      if (!cancelled) setSettingsLoaded(true);
    });
    return () => { cancelled = true; };
  }, [desktop]);

  useEffect(() => {
    try { window.localStorage.setItem("opnlauncher.playtime-range", range); } catch { /* The range remains usable for this session. */ }
  }, [range]);

  useEffect(() => {
    if (!desktop || !settingsLoaded) return;
    let cancelled = false;
    const load = async () => {
      const windowRange = createPlaytimeWindow(range);
      setLoading(true);
      try {
        const result = await invoke<PlaytimeSummary>("get_playtime_summary", {
          startAt: windowRange.startAt,
          endAt: windowRange.endAt,
        });
        if (!cancelled) {
          setSummary(result);
          setNow(new Date());
          setError("");
        }
      } catch (reason) {
        if (!cancelled) setError(String(reason));
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    void load();
    if (!trackingEnabled) return () => { cancelled = true; };
    const refreshTimer = window.setInterval(() => { void load(); }, 30_000);
    return () => {
      cancelled = true;
      window.clearInterval(refreshTimer);
    };
  }, [desktop, range, settingsLoaded, trackingEnabled]);

  const chart = useMemo(() => aggregatePlaytime(summary?.sessions ?? [], range, now), [now, range, summary]);
  const periodSeconds = chart.buckets.reduce((total, bucket) => total + bucket.seconds, 0);
  const maxBucketSeconds = Math.max(0, ...chart.buckets.map((bucket) => bucket.seconds));
  const hasChartData = maxBucketSeconds > 0;

  async function changeTracking() {
    if (!desktop || savingSetting || !settingsLoaded) return;
    const nextValue = !trackingEnabled;
    setSavingSetting(true);
    setError("");
    try {
      await invoke("set_playtime_tracking_enabled", { enabled: nextValue });
      setTrackingEnabled(nextValue);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSavingSetting(false);
    }
  }

  return (
    <section className="playtime-card" aria-labelledby="playtime-title">
      <div className="playtime-card-heading">
        <div className="playtime-card-title">
          <span className="playtime-card-icon"><ChartColumnIncreasing size={18} /></span>
          <div><p className="eyebrow">LOCAL PLAY HISTORY</p><h2 id="playtime-title">Playtime</h2><p>See how much time you spend in your game profiles.</p></div>
        </div>
        <div className="playtime-card-controls">
          <label className="playtime-tracking-toggle">
            <span>{trackingEnabled ? "Tracking on" : "Track playtime"}</span>
            <input type="checkbox" checked={trackingEnabled} onChange={() => void changeTracking()} disabled={!desktop || !settingsLoaded || savingSetting} aria-label="Track playtime on this device" />
            <span className="playtime-switch" aria-hidden="true">{savingSetting ? <LoaderCircle className="spin" size={13} /> : null}</span>
          </label>
          <label className="playtime-range-control"><span className="sr-only">Chart time range</span><select value={range} onChange={(event) => setRange(event.target.value as PlaytimeRange)} aria-label="Playtime chart time range">{rangeOptions.map((option) => <option value={option.id} key={option.id}>{option.label}</option>)}</select></label>
        </div>
      </div>

      {!desktop && <div className="playtime-empty-state"><Clock3 size={18} /><span>Open OpnLauncher as a desktop app to track local game sessions.</span></div>}
      {desktop && !settingsLoaded && <div className="playtime-empty-state"><LoaderCircle className="spin" size={18} /><span>Loading your local playtime preference…</span></div>}
      {desktop && settingsLoaded && <>
        {error && <div className="error-banner playtime-error" role="alert"><CircleAlert size={15} /><span>{error}</span></div>}
        <div className="playtime-stats">
          <article><span>Selected range</span><strong>{formatPlaytime(periodSeconds)}</strong></article>
          <article><span>Sessions in range</span><strong>{summary?.sessions.length ?? 0}</strong></article>
          <article><span>Lifetime playtime</span><strong>{formatPlaytime(summary?.lifetimeSeconds ?? 0)}</strong></article>
        </div>
        {loading && !summary ? <div className="playtime-empty-state"><LoaderCircle className="spin" size={18} /><span>Loading play history…</span></div> : hasChartData ? (
          <div className={`playtime-chart-wrap range-${range}`}>
            <div className="playtime-chart" role="img" aria-label={`${rangeOptions.find((option) => option.id === range)?.label} playtime chart. ${formatPlaytime(periodSeconds)} across ${summary?.sessions.length ?? 0} sessions.`}>
              {chart.buckets.map((bucket) => {
                const barHeight = maxBucketSeconds > 0 ? Math.max(2, bucket.seconds / maxBucketSeconds * 100) : 0;
                return <div className="playtime-bar-cell" key={bucket.key} title={`${bucket.title}: ${formatPlaytime(bucket.seconds)}`}>
                  <div className="playtime-bar-track"><span className={bucket.seconds > 0 ? "has-playtime" : ""} style={{ height: `${barHeight}%` }} /></div>
                  <span className="playtime-bar-label">{bucket.label}</span>
                </div>;
              })}
            </div>
            <div className="playtime-chart-caption"><span><span className="playtime-legend-dot" /> Time played</span><span>{loading ? "Refreshing…" : `${rangeOptions.find((option) => option.id === range)?.label} · your local time`}</span></div>
          </div>
        ) : <div className="playtime-empty-state"><Clock3 size={18} /><span>{trackingEnabled ? "No play sessions in this range yet. Launch an instance to start your chart." : "Playtime tracking is off. Turn it on to record future sessions on this device."}</span></div>}
        <p className="playtime-privacy-note"><ShieldCheck size={14} /> Play sessions stay in OpnLauncher’s local database. Nothing is uploaded. Tracking can be turned off at any time.</p>
      </>}
    </section>
  );
}
