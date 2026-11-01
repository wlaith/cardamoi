import { isTyping } from "../keys";

export type PlayerAction =
  | { kind: "mpv"; args: (string | number)[]; notice?: "subtitle" | "delay" }
  | { kind: "fullscreen" }
  | { kind: "escape" }
  | { kind: "next" };

const mpv = (...args: (string | number)[]): PlayerAction => ({ kind: "mpv", args });

/** The keys of mpv-input.conf, which people already know, as player actions. */
export function playerKey(e: { key: string; target: EventTarget | null; metaKey: boolean; ctrlKey: boolean }): PlayerAction | null {
  if (e.metaKey || e.ctrlKey || isTyping(e.target)) return null;
  switch (e.key.length === 1 ? e.key.toLowerCase() : e.key) {
    case " ":
    case "k":
      return mpv("cycle", "pause");
    case "ArrowLeft":
    case "j":
      return mpv("seek", -10);
    case "ArrowRight":
    case "l":
      return mpv("seek", 10);
    case "ArrowUp":
      return mpv("add", "volume", 5);
    case "ArrowDown":
      return mpv("add", "volume", -5);
    case "m":
      return mpv("cycle", "mute");
    case "Home":
      return mpv("seek", 0, "absolute");
    case "c":
      return { kind: "mpv", args: ["cycle", "sub"], notice: "subtitle" };
    case "z":
      return { kind: "mpv", args: ["add", "sub-delay", -0.1], notice: "delay" };
    case "x":
      return { kind: "mpv", args: ["add", "sub-delay", 0.1], notice: "delay" };
    case "f":
      return { kind: "fullscreen" };
    case "Enter":
      return { kind: "next" };
    case "Escape":
      return { kind: "escape" };
    default:
      return null;
  }
}
