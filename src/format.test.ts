import { describe, expect, it, vi } from "vitest";
import { currentDateLabel, formatRelativeTime, javaLabel } from "./format";

describe("launcher labels", () => {
  it("formats a locally stored last-played timestamp", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-22T12:00:00Z"));
    expect(formatRelativeTime(Date.parse("2026-10-22T11:30:00Z") / 1000)).toBe("30 min ago");
    expect(formatRelativeTime(null)).toBe("Never played");
    vi.useRealTimers();
  });

  it("labels the detected Java major version", () => {
    expect(javaLabel(21)).toBe("Java 21");
  });

  it("formats the current date using the device locale", () => {
    expect(currentDateLabel(new Date("2026-10-22T12:00:00Z"))).toContain("OCTOBER");
  });
});
