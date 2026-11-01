import { describe, expect, it } from "vitest";
import { controlsVisible } from "./controls";

const base = { status: "playing", paused: false, buffering: false, menuOpen: false, pointerOnControls: false };

describe("controlsVisible", () => {
  it("hides after 3 seconds without movement", () => {
    expect(controlsVisible({ ...base, idleMs: 2999 })).toBe(true);
    expect(controlsVisible({ ...base, idleMs: 3000 })).toBe(false);
  });
  it("stays while paused, opening, buffering, with a menu open or the pointer on the controls", () => {
    for (const hold of [{ paused: true }, { status: "opening" }, { buffering: true }, { menuOpen: true }, { pointerOnControls: true }]) {
      expect(controlsVisible({ ...base, ...hold, idleMs: 60000 })).toBe(true);
    }
  });
  it("stays when playback ended or failed, so there is a way back", () => {
    expect(controlsVisible({ ...base, status: "ended", idleMs: 60000 })).toBe(true);
    expect(controlsVisible({ ...base, status: "failed", idleMs: 60000 })).toBe(true);
  });
});
