import { describe, expect, it } from "vitest";
import { aggregatePlaytime, createPlaytimeWindow, formatPlaytime } from "./playtime";
import type { PlaytimeSession } from "./types";

function session(start: Date, end: Date): PlaytimeSession {
  return {
    id: "session-1",
    instanceId: "instance-1",
    startedAt: Math.floor(start.getTime() / 1000),
    lastUpdatedAt: Math.floor(end.getTime() / 1000),
    endedAt: Math.floor(end.getTime() / 1000),
    durationSeconds: Math.floor((end.getTime() - start.getTime()) / 1000),
  };
}

describe("playtime chart aggregation", () => {
  it("splits a session across local calendar days", () => {
    const start = new Date(2026, 5, 1, 23, 30);
    const end = new Date(2026, 5, 2, 0, 30);
    const chart = aggregatePlaytime([session(start, end)], "7d", new Date(2026, 5, 2, 1));
    const yesterday = chart.buckets.find((bucket) => bucket.key === "2026-06-01");
    const today = chart.buckets.find((bucket) => bucket.key === "2026-06-02");
    expect(yesterday?.seconds).toBe(30 * 60);
    expect(today?.seconds).toBe(30 * 60);
  });

  it("clips earlier sessions to the selected window", () => {
    const now = new Date(2026, 5, 8, 12);
    const window = createPlaytimeWindow("7d", now);
    const start = new Date(window.startAt * 1000);
    start.setHours(start.getHours() - 2);
    const end = new Date(window.startAt * 1000);
    end.setHours(end.getHours() + 4);
    const chart = aggregatePlaytime([session(start, end)], "7d", now);
    expect(chart.buckets.reduce((total, bucket) => total + bucket.seconds, 0)).toBe(4 * 60 * 60);
  });

  it("uses readable weekly and monthly chart buckets", () => {
    const now = new Date(2026, 10, 4, 12);
    expect(createPlaytimeWindow("90d", now).buckets).toHaveLength(13);
    expect(createPlaytimeWindow("12m", now).buckets).toHaveLength(12);
  });
});

describe("formatPlaytime", () => {
  it("formats minutes, hours, days, and short sessions", () => {
    expect(formatPlaytime(0)).toBe("0m");
    expect(formatPlaytime(35)).toBe("<1m");
    expect(formatPlaytime(3660)).toBe("1h 1m");
    expect(formatPlaytime(90_000)).toBe("1d 1h");
  });
});
