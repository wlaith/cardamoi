import { ref } from "vue";
import type { LocalEpisode } from "./library";

export interface AlertButton {
  label: string;
  value: string;
  kind?: "primary" | "danger";
}

export type Sheet =
  | {
      kind: "alert";
      title: string;
      message: string;
      details?: string;
      buttons: AlertButton[];
      resolve: (value: string | null) => void;
    }
  | { kind: "info"; ep: LocalEpisode }
  | { kind: "magnet"; link: string };

export const sheet = ref<Sheet | null>(null);

export function alertSheet(opts: { title: string; message: string; details?: string; buttons?: AlertButton[] }) {
  closeSheet();
  return new Promise<string | null>((resolve) => {
    sheet.value = {
      kind: "alert",
      buttons: [{ label: "OK", value: "ok", kind: "primary" }],
      ...opts,
      resolve,
    };
  });
}

export function closeSheet(value: string | null = null) {
  const open = sheet.value;
  sheet.value = null;
  if (open?.kind === "alert") open.resolve(value);
}

export const palette = ref(false);

export const downloadsOpen = ref(false);

export type Hud = { kind: "captions"; ep: LocalEpisode } | { kind: "nowPlaying" };
export const hud = ref<Hud | null>(null);

export type MenuEntry = { label: string; action: () => void; disabled?: boolean; danger?: boolean } | "separator";
export const contextMenu = ref<{ x: number; y: number; items: MenuEntry[]; above?: boolean } | null>(null);

export function openMenu(event: MouseEvent, items: MenuEntry[]) {
  event.preventDefault();
  contextMenu.value = { x: event.clientX, y: event.clientY, items };
}

export function openMenuBelow(el: HTMLElement, items: MenuEntry[], above = false) {
  const box = el.getBoundingClientRect();
  contextMenu.value = { x: box.left, y: above ? box.top : box.bottom, items, above };
}
