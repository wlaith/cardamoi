/** 67 becomes "1:07", 3723 becomes "1:02:03". */
export function formatTime(secs: number): string {
  const s = Number.isFinite(secs) ? Math.max(0, Math.floor(secs)) : 0;
  const [h, m, sec] = [Math.floor(s / 3600), Math.floor(s / 60) % 60, s % 60];
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${m}:${pad(sec)}`;
}

/** Time left, with a true minus sign: "−21:48". */
export function remaining(position: number, duration: number): string {
  return `−${formatTime(duration - position)}`;
}
