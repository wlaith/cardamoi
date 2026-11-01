// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import { playerKey } from "./keys";

const k = (key: string, target: EventTarget | null = null) => playerKey({ key, target, metaKey: false, ctrlKey: false });

describe("playerKey", () => {
  it("maps the keys mpv taught people", () => {
    expect(k(" ")).toEqual({ kind: "mpv", args: ["cycle", "pause"] });
    expect(k("k")).toEqual({ kind: "mpv", args: ["cycle", "pause"] });
    expect(k("j")).toEqual({ kind: "mpv", args: ["seek", -10] });
    expect(k("ArrowLeft")).toEqual({ kind: "mpv", args: ["seek", -10] });
    expect(k("l")).toEqual({ kind: "mpv", args: ["seek", 10] });
    expect(k("ArrowRight")).toEqual({ kind: "mpv", args: ["seek", 10] });
    expect(k("ArrowUp")).toEqual({ kind: "mpv", args: ["add", "volume", 5] });
    expect(k("ArrowDown")).toEqual({ kind: "mpv", args: ["add", "volume", -5] });
    expect(k("m")).toEqual({ kind: "mpv", args: ["cycle", "mute"] });
    expect(k("Home")).toEqual({ kind: "mpv", args: ["seek", 0, "absolute"] });
  });
  it("tells subtitle keys to show a notice", () => {
    expect(k("c")).toEqual({ kind: "mpv", args: ["cycle", "sub"], notice: "subtitle" });
    expect(k("z")).toEqual({ kind: "mpv", args: ["add", "sub-delay", -0.1], notice: "delay" });
    expect(k("x")).toEqual({ kind: "mpv", args: ["add", "sub-delay", 0.1], notice: "delay" });
  });
  it("maps window actions", () => {
    expect(k("f")).toEqual({ kind: "fullscreen" });
    expect(k("Enter")).toEqual({ kind: "next" });
    expect(k("Escape")).toEqual({ kind: "escape" });
  });
  it("ignores keys it doesn't know", () => {
    expect(k("q")).toBeNull();
  });
  it("player_keys_ignore_typing: leaves keys to a focused field", () => {
    expect(k(" ", document.createElement("input"))).toBeNull();
    expect(k("f", document.createElement("textarea"))).toBeNull();
  });
  it("leaves shortcuts with a modifier to the app", () => {
    expect(playerKey({ key: "f", target: null, metaKey: true, ctrlKey: false })).toBeNull();
    expect(playerKey({ key: "k", target: null, metaKey: false, ctrlKey: true })).toBeNull();
  });
});
