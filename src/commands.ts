import { api, episodeLabel, isMpvMissing, magnetName, plainError, type CatalogTitle, type LibraryEntry, type NameLookup } from "./api";
import { nextEpisode, showKey, type LocalEpisode, type Show } from "./library";
import { go } from "./nav";
import { announce, hideEntries, refresh, selection, showEntries, startOpening, stopOpening, type Selected } from "./store";
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

export function moveToTrash(list: LibraryEntry | LibraryEntry[]) {
  const all = Array.isArray(list) ? list : [list];
  if (!all.length) return;
  const ids = all.map((e) => e.id);
  hideEntries(ids);
  const sel = selection.value;
  const selectedId = sel?.kind === "video" ? sel.ep.entry.id : null;
  if ((selectedId != null && ids.includes(selectedId)) || (sel?.kind === "show" && sel.show.entries.some((e) => ids.includes(e.id)))) {
    selection.value = null;
  }
  const notStarted = all.every((e) => e.videos.length === 0);
  const what = all.length === 1 ? `"${all[0].title}"` : `${all.length} items`;
  announce(
    notStarted ? `Cancelled ${what}` : `Moved ${what} to the Trash`,
    async () => showEntries(ids),
    async () => {
      for (const e of all) {
        try {
          await api.remove(e.id);
        } catch (err) {
          showEntries([e.id]);
          showError(`"${e.title}" couldn't be removed.`, err);
        }
      }
      showEntries(ids);
    },
  );
}

export async function setManyWatched(eps: LocalEpisode[], watched: boolean) {
  if (eps.length === 1) return setWatched(eps[0], watched);
  const before = eps.map((ep) => [ep, ep.video.watched] as const);
  const ok = await attempt("The watched marks couldn't be changed.", () =>
    Promise.all(eps.map((ep) => api.markWatched(ep.entry.id, ep.video.fileIndex, watched))),
  );
  if (ok) {
    announce(`Marked ${eps.length} as ${watched ? "watched" : "unwatched"}`, () =>
      Promise.all(before.map(([ep, was]) => api.markWatched(ep.entry.id, ep.video.fileIndex, was))),
    );
  }
}

/** `title` is what the user confirmed it is on TMDB; `at` is the episode read from its name. */
export async function addMagnet(
  link: string,
  title: CatalogTitle | null = null,
  at: Pick<NameLookup, "season" | "episode"> | null = null,
): Promise<string | null> {
  const show = title?.kind === "tv";
  try {
    await api.addDownload({
      title: title?.title ?? magnetName(link),
      year: title?.year ?? null,
      source: "Magnet link",
      torrentUrl: link,
      poster: title?.poster ?? null,
      singleVideo: false,
      tmdbId: title?.id ?? null,
      kind: title?.kind ?? null,
      season: show ? (at?.season ?? null) : null,
      episode: show ? (at?.episode ?? null) : null,
    });
    await refresh();
    announce(`Added "${title?.title || magnetName(link) || "magnet link"}" to Downloads`);
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
  items.push({ label: "Move to Trash", action: () => moveToTrash(e), danger: true });
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
