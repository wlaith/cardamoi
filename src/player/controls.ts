/** How long the controls stay after the last pointer move or key press, in ms. */
export const HIDE_AFTER = 3000;

export interface ControlsState {
  status: string;
  paused: boolean;
  buffering: boolean;
  menuOpen: boolean;
  pointerOnControls: boolean;
  idleMs: number;
}

/** The controls get out of the way while a video plays, and stay whenever there's something to do or to read. */
export function controlsVisible(s: ControlsState): boolean {
  if (s.status !== "playing" || s.paused || s.buffering || s.menuOpen || s.pointerOnControls) return true;
  return s.idleMs < HIDE_AFTER;
}
