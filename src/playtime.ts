import type { PlaytimeSession } from "./types";

export type PlaytimeRange = "7d" | "30d" | "90d" | "12m";

export interface PlaytimeBucket {
  key: string;
  label: string;
  title: string;
  startAt: number;
  endAt: number;
  seconds: number;
}

export interface PlaytimeWindow {
  startAt: number;
  endAt: number;
  buckets: PlaytimeBucket[];
}

function dateKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

function monthKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
}

function localDayNumber(date: Date): number {
  return Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / 86_400_000;
}

function localMidnight(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

export function createPlaytimeWindow(range: PlaytimeRange, now = new Date()): PlaytimeWindow {
  const endAt = Math.floor(now.getTime() / 1000);
  const buckets: PlaytimeBucket[] = [];
  const today = localMidnight(now);

  if (range === "7d" || range === "30d" || range === "90d") {
    const days = range === "7d" ? 7 : range === "30d" ? 30 : 90;
    const startDate = new Date(today);
    startDate.setDate(startDate.getDate() - (days - 1));
    const count = range === "90d" ? Math.ceil(days / 7) : days;
    const step = range === "90d" ? 7 : 1;

    for (let index = 0; index < count; index += 1) {
      const bucketStart = new Date(startDate);
      bucketStart.setDate(bucketStart.getDate() + index * step);
      const bucketEnd = new Date(bucketStart);
      bucketEnd.setDate(bucketEnd.getDate() + step);
      const title = bucketStart.toLocaleDateString(undefined, range === "90d"
        ? { month: "short", day: "numeric", year: "numeric" }
        : { weekday: "long", month: "short", day: "numeric", year: "numeric" });
      const showDailyLabel = range === "7d" || index % 5 === 0 || index === count - 1;
      const label = range === "7d"
        ? bucketStart.toLocaleDateString(undefined, { weekday: "short" })
        : range === "30d" && showDailyLabel
          ? bucketStart.toLocaleDateString(undefined, { month: "short", day: "numeric" })
          : range === "90d"
            ? bucketStart.toLocaleDateString(undefined, { month: "short", day: "numeric" })
            : "";
      buckets.push({
        key: dateKey(bucketStart),
        label,
        title,
        startAt: Math.floor(bucketStart.getTime() / 1000),
        endAt: Math.floor(bucketEnd.getTime() / 1000),
        seconds: 0,
      });
    }
    return { startAt: buckets[0]?.startAt ?? endAt, endAt, buckets };
  }

  const firstMonth = new Date(today.getFullYear(), today.getMonth() - 11, 1);
  for (let index = 0; index < 12; index += 1) {
    const bucketStart = new Date(firstMonth.getFullYear(), firstMonth.getMonth() + index, 1);
    const bucketEnd = new Date(firstMonth.getFullYear(), firstMonth.getMonth() + index + 1, 1);
    buckets.push({
      key: monthKey(bucketStart),
      label: bucketStart.toLocaleDateString(undefined, { month: "short" }),
      title: bucketStart.toLocaleDateString(undefined, { month: "long", year: "numeric" }),
      startAt: Math.floor(bucketStart.getTime() / 1000),
      endAt: Math.floor(bucketEnd.getTime() / 1000),
      seconds: 0,
    });
  }
  return { startAt: buckets[0].startAt, endAt, buckets };
}

export function aggregatePlaytime(sessions: readonly PlaytimeSession[], range: PlaytimeRange, now = new Date()): PlaytimeWindow {
  const window = createPlaytimeWindow(range, now);
  const bucketByKey = new Map(window.buckets.map((bucket, index) => [bucket.key, index]));
  const firstBucket = window.buckets[0];

  for (const session of sessions) {
    let cursor = Math.max(session.startedAt, window.startAt);
    const finishAt = Math.min(session.endedAt ?? session.lastUpdatedAt, window.endAt);
    while (cursor < finishAt) {
      const cursorDate = new Date(cursor * 1000);
      const nextMidnight = new Date(cursorDate.getFullYear(), cursorDate.getMonth(), cursorDate.getDate() + 1);
      const segmentEnd = Math.min(finishAt, Math.floor(nextMidnight.getTime() / 1000));
      let bucketIndex: number | undefined;
      if (range === "7d" || range === "30d") {
        bucketIndex = bucketByKey.get(dateKey(cursorDate));
      } else if (range === "90d" && firstBucket) {
        bucketIndex = Math.floor((localDayNumber(cursorDate) - localDayNumber(new Date(firstBucket.startAt * 1000))) / 7);
      } else {
        bucketIndex = bucketByKey.get(monthKey(cursorDate));
      }
      if (bucketIndex !== undefined && window.buckets[bucketIndex]) {
        window.buckets[bucketIndex].seconds += segmentEnd - cursor;
      }
      if (segmentEnd <= cursor) break;
      cursor = segmentEnd;
    }
  }
  return window;
}

export function formatPlaytime(seconds: number): string {
  const minutes = Math.floor(Math.max(0, seconds) / 60);
  if (seconds > 0 && minutes === 0) return "<1m";
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  if (days > 0) return `${days}d ${hours % 24}h`;
  if (hours > 0) return `${hours}h ${minutes % 60}m`;
  return `${minutes}m`;
}
