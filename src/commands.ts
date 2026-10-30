import { api, episodeLabel, isMpvMissing, magnetName, plainError, type LibraryEntry } from "./api";
import { nextEpisode, showKey, type LocalEpisode, type Show } from "./library";
import { go } from "./nav";
import { announce, refresh, selection, startOpening, stopOpening, type Selected } from "./store";
import { alertSheet, hud, sheet, type MenuEntry } from "./ui";

export function videoLabel(ep: LocalEpisode) {
  const label = episodeLabel(ep.video.season, ep.video.episode);
  return label ? `${ep.entry.title} ${label}` : ep.entry.title;
}

export async function showError(title: string, e: unknown, ep?: LocalEpisode) {
  const { message, details } = plainError(e);
  if (ep && isMpvMissing(e)) {
    const choice = await alertSheet({
      title,
      message,
      buttons: [
        { label: "Cancel", value: "cancel" },
        { label: "Open in System Player", value: "external", kind: "primary" },
      ],
    });
    if (choice === "external") openExternal(ep);
    return;
  }
  await alertSheet({ title, message, details });
}

async function attempt(title: string, action: () => Promise<unknown>, ep?: LocalEpisode) {
  try {
    await action();
    await refresh();
    return true;
  } catch (e) {
    showError(title, e, ep);
    return false;
  }
}

export async function play(ep: LocalEpisode, fromStart = false) {
  startOpening(ep.entry.id, ep.video.fileIndex, videoLabel(ep));
  const ok = await attempt(
    `"${videoLabel(ep)}" couldn't be played.`,
    () => api.play(ep.entry.id, ep.video.fileIndex, fromStart),
    ep,
  );
  if (!ok) stopOpening();
}

export function openExternal(ep: LocalEpisode) {
  attempt("The system player couldn't open this video.", () => api.openExternal(ep.entry.id, ep.video.fileIndex));
}

export function stop() {
  attempt("Playback couldn't be stopped.", () => api.stopPlayer());
}

export function togglePause() {
  attempt("Playback couldn't be paused.", () => api.playerCommand(["cycle", "pause"]));
}

export async function setWatched(ep: LocalEpisode, watched: boolean) {
  const was = ep.video.watched;
  const ok = await attempt("The watched mark couldn't be changed.", () =>
    api.markWatched(ep.entry.id, ep.video.fileIndex, watched),
  );
  if (ok) {
    announce(`Marked "${videoLabel(ep)}" as ${watched ? "watched" : "unwatched"}`, () =>
      api.markWatched(ep.entry.id, ep.video.fileIndex, was),
    );
  }
}

export async function setSeasonWatched(tmdbId: number, season: number, episodes: LocalEpisode[], watched: boolean) {
  const before = episodes.map((ep) => [ep, ep.video.watched] as const);
  const ok = await attempt("The season couldn't be marked.", () => api.markSeasonWatched(tmdbId, season, watched));
  if (ok) {
    announce(`Marked season ${season} as ${watched ? "watched" : "unwatched"}`, () =>
      Promise.all(before.map(([ep, was]) => api.markWatched(ep.entry.id, ep.video.fileIndex, was))),
    );
  }
}

export function reveal(entry: LibraryEntry) {
  attempt("The files couldn't be shown in Finder.", () => api.reveal(entry.id));
}

export function pause(entry: LibraryEntry) {
  attempt("The download couldn't be paused.", () => api.pause(entry.id));
}

export function resume(entry: LibraryEntry) {
  attempt("The download couldn't be resumed.", () => api.resume(entry.id));
}

export function retry(entry: LibraryEntry) {
  attempt("The download couldn't be restarted.", () => api.retryDownload(entry.id));
}

export async function moveToTrash(entry: LibraryEntry) {
  const notStarted = entry.videos.length === 0;
  const count = entry.videos.length;
  const message = notStarted
    ? "Nothing has been downloaded yet, so there are no files to remove."
    : count > 1
      ? `This download holds ${count} episodes. All of them are moved to the Trash, and you can put them back from there.`
      : "The video file is moved to the Trash. You can put it back from there.";
  const choice = await alertSheet({
    title: notStarted ? `Cancel "${entry.title}"?` : `Move "${entry.title}" to the Trash?`,
    message,
    buttons: [
      { label: notStarted ? "Keep Waiting" : "Cancel", value: "cancel" },
      { label: notStarted ? "Cancel Download" : "Move to Trash", value: "trash", kind: "danger" },
    ],
  });
  if (choice !== "trash") return;
  if (await attempt(`"${entry.title}" couldn't be removed.`, () => api.remove(entry.id))) {
    if (selection.value?.kind === "video" && selection.value.ep.entry.id === entry.id) selection.value = null;
    announce(notStarted ? `Cancelled "${entry.title}"` : `Moved "${entry.title}" to the Trash`);
  }
}

export async function addMagnet(link: string): Promise<string | null> {
  try {
    await api.addDownload({
      title: magnetName(link),
      year: null,
      source: "Magnet link",
      torrentUrl: link,
      poster: null,
      singleVideo: false,
    });
    await refresh();
    announce(`Added "${magnetName(link) || "magnet link"}" to Downloads`);
    return null;
  } catch (e) {
    return plainError(e).message;
  }
}

export function getInfo(ep: LocalEpisode) {
  sheet.value = { kind: "info", ep };
}

export function subtitles(ep: LocalEpisode) {
  hud.value = { kind: "captions", ep };
}

export async function checkFolder() {
  try {
    const added = await api.scanLibraryFolder();
    announce(added ? `Added ${added} ${added === 1 ? "video" : "videos"} from the folder` : "No new videos in the folder");
    await refresh();
  } catch (e) {
    showError("The folder couldn't be checked.", e);
  }
}

export function openFolder() {
  attempt("The folder couldn't be opened.", () => api.openLibraryFolder());
}

export function videoOf(s: Selected | null): LocalEpisode | null {
  if (!s) return null;
  return s.kind === "video" ? s.ep : nextEpisode(s.show);
}

export function openShow(show: Show) {
  if (show.tmdbId == null) {
    go({ name: "localShow", key: showKey(show) });
    return;
  }
  go({
    name: "title",
    title: {
      id: show.tmdbId,
      kind: "tv",
      title: show.title,
      year: show.year,
      poster: show.poster,
      backdrop: null,
      overview: "",
      rating: null,
    },
  });
}

export function openFilm(ep: LocalEpisode) {
  const e = ep.entry;
  if (e.tmdb_id == null) {
    getInfo(ep);
    return;
  }
  go({
    name: "title",
    title: { id: e.tmdb_id, kind: "movie", title: e.title, year: e.year, poster: e.poster, backdrop: null, overview: "", rating: null },
  });
}

export function videoMenu(ep: LocalEpisode, opts: { open?: () => void } = {}): MenuEntry[] {
  const v = ep.video;
  const e = ep.entry;
  const items: MenuEntry[] = [
    {
      label: v.resumeAt != null ? "Continue" : v.watched ? "Play Again" : "Play",
      action: () => play(ep, v.watched),
      disabled: !v.playable,
    },
  ];
  if (v.resumeAt != null) items.push({ label: "Play from Beginning", action: () => play(ep, true) });
  if (opts.open) items.push({ label: "Open", action: opts.open });
  items.push(
    "separator",
    { label: v.watched ? "Mark as Unwatched" : "Mark as Watched", action: () => setWatched(ep, !v.watched) },
    { label: "Subtitles…", action: () => subtitles(ep) },
    { label: "Get Info", action: () => getInfo(ep) },
    { label: "Show in Finder", action: () => reveal(e), disabled: e.status === "missing" },
    { label: "Open in System Player", action: () => openExternal(ep), disabled: !v.playable },
    "separator",
  );
  if (e.status === "downloading" || e.status === "starting") items.push({ label: "Pause Download", action: () => pause(e) });
  else if (e.status === "paused") items.push({ label: "Resume Download", action: () => resume(e) });
  else if (e.status === "error") items.push({ label: "Try Download Again", action: () => retry(e) });
  if (items[items.length - 1] !== "separator") items.push("separator");
  items.push({ label: "Move to Trash…", action: () => moveToTrash(e), danger: true });
  return items;
}

export function showMenu(show: Show): MenuEntry[] {
  const next = nextEpisode(show);
  return [
    {
      label: next ? `${next.video.resumeAt != null ? "Continue" : "Play"} ${episodeLabel(next.video.season, next.video.episode)}` : "Play",
      action: () => next && play(next),
      disabled: !next,
    },
    { label: "Open", action: () => openShow(show) },
  ];
}
