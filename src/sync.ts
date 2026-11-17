import type { LibraryEntry } from "./api";

export const ACTIVE_MS = 1000;
export const IDLE_MS = 10_000;

const ACTIVE = new Set(["resolving", "starting", "downloading"]);

export function refreshDelay(entries: Pick<LibraryEntry, "status">[], playing: boolean): number {
  return playing || entries.some((e) => ACTIVE.has(e.status)) ? ACTIVE_MS : IDLE_MS;
}
