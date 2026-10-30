import { episodeLabel, formatBytes, timeLeft, type LibraryEntry } from "./api";
import { filmVideo, nextEpisode, showSummary, type Show } from "./library";

export function resolvingText(e: LibraryEntry): string {
  return (e.resolvingSecs ?? 0) < 120
    ? "Getting the file list from peers"
    : "Still looking for peers. Rare torrents can take a few minutes.";
}

export function canRetry(e: LibraryEntry): boolean {
  return e.status === "error" && !!e.torrent_url?.startsWith("magnet:");
}

export function readyIn(secs: number | null): string {
  if (secs == null) return "";
  return secs < 60 ? "Ready in under a minute" : `Ready in about ${Math.round(secs / 60)} min`;
}

export function percent(e: LibraryEntry) {
  return Math.floor(e.progress * 100);
}

export function downloadStatus(e: LibraryEntry): string {
  const video = e.videos[0];
  switch (e.status) {
    case "resolving":
      return resolvingText(e);
    case "starting":
      return "Finding peers";
    case "paused":
      return `Paused at ${percent(e)} %`;
    case "missing":
      return "Missing from the downloads folder";
    case "error":
      return e.error ? `Failed: ${e.error}` : "Download failed";
    case "done":
      return `Downloaded, ${formatBytes(e.totalBytes)}`;
    case "downloading": {
      const parts = [`${formatBytes(e.downloadedBytes)} of ${formatBytes(e.totalBytes)}`, `${e.speedMibps.toFixed(1)} MB/s`];
      if (e.eta) parts.push(`${e.eta} left`);
      if (e.videos.length === 1 && video?.playable) parts.push("Ready to watch");
      else if (e.videos.length === 1 && video?.readyInSecs) parts.push(readyIn(video.readyInSecs));
      return parts.join(" · ");
    }
  }
}

export function filmLine(e: LibraryEntry): { text: string; accent: boolean } {
  const ep = filmVideo(e);
  const v = ep?.video;
  const year = e.year ? String(e.year) : "";
  const join = (...parts: string[]) => parts.filter(Boolean).join(" · ");
  if (e.status === "done" || e.status === "missing") {
    if (e.status === "missing") return { text: "Missing from the downloads folder", accent: false };
    if (v?.watched) return { text: join(year, "Watched"), accent: false };
    if (v?.resumeAt != null) return { text: join(year, timeLeft(v)), accent: false };
    return { text: year || formatBytes(e.totalBytes), accent: false };
  }
  if (e.status === "downloading" && v?.playable) return { text: join(`${percent(e)} %`, "Ready to watch"), accent: true };
  if (e.status === "downloading") return { text: join(`${percent(e)} %`, readyIn(v?.readyInSecs ?? null)), accent: false };
  if (e.status === "resolving") return { text: "Getting the file list", accent: false };
  if (e.status === "error") return { text: "Download failed", accent: false };
  return { text: downloadStatus(e), accent: false };
}

export function showLine(show: Show): string {
  const next = nextEpisode(show);
  const s = showSummary(show);
  const parts: string[] = [];
  if (next) parts.push(`Next: ${episodeLabel(next.video.season, next.video.episode)}`);
  else if (s.total && s.watched === s.total) parts.push("All caught up");
  if (s.unwatched > 1) parts.push(`${s.unwatched} unwatched`);
  if (s.downloading) parts.push(`${s.downloading} downloading`);
  return parts.join(" · ");
}
