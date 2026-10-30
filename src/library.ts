import type { LibraryEntry, VideoEntry } from "./api";

export interface LocalEpisode {
  entry: LibraryEntry;
  video: VideoEntry;
}

export interface Show {
  tmdbId: number | null;
  title: string;
  year: number | null;
  poster: string | null;
  episodes: LocalEpisode[];
  pending: LibraryEntry[];
  entries: LibraryEntry[];
  lastActivity: number;
}

export type Shelf = { kind: "film"; entry: LibraryEntry; lastActivity: number } | { kind: "show"; show: Show; lastActivity: number };

export function episodeKey(season: number | null, episode: number | null) {
  return `${season ?? 0}:${episode ?? 0}`;
}

function isShow(e: LibraryEntry) {
  return e.kind === "tv" || (e.kind == null && e.season != null);
}

function activity(e: LibraryEntry) {
  return Math.max(e.added_at, ...e.videos.map((v) => v.lastPlayedAt ?? 0));
}

function better(a: LocalEpisode, b: LocalEpisode) {
  if (a.video.playable !== b.video.playable) return a.video.playable ? a : b;
  return a.video.positionSecs >= b.video.positionSecs ? a : b;
}

export function groupLibrary(entries: LibraryEntry[]): Shelf[] {
  const shows = new Map<string, Show>();
  const shelf: Shelf[] = [];
  for (const entry of entries) {
    if (!isShow(entry)) {
      shelf.push({ kind: "film", entry, lastActivity: activity(entry) });
      continue;
    }
    const key = entry.tmdb_id != null ? `tmdb:${entry.tmdb_id}` : `title:${entry.title.toLowerCase()}`;
    let show = shows.get(key);
    if (!show) {
      show = {
        tmdbId: entry.tmdb_id,
        title: entry.title,
        year: entry.year,
        poster: entry.poster,
        episodes: [],
        pending: [],
        entries: [],
        lastActivity: 0,
      };
      shows.set(key, show);
    }
    show.entries.push(entry);
    show.poster ??= entry.poster;
    show.lastActivity = Math.max(show.lastActivity, activity(entry));
    if (entry.videos.length === 0) show.pending.push(entry);
    for (const video of entry.videos) show.episodes.push({ entry, video });
  }
  for (const show of shows.values()) {
    const byEpisode = new Map<string, LocalEpisode>();
    for (const ep of show.episodes) {
      const key = episodeKey(ep.video.season, ep.video.episode);
      const kept = byEpisode.get(key);
      byEpisode.set(key, kept ? better(kept, ep) : ep);
    }
    show.episodes = [...byEpisode.values()].sort(
      (a, b) => (a.video.season ?? 0) - (b.video.season ?? 0) || (a.video.episode ?? 0) - (b.video.episode ?? 0),
    );
    shelf.push({ kind: "show", show, lastActivity: show.lastActivity });
  }
  return shelf.sort((a, b) => b.lastActivity - a.lastActivity);
}

export function nextEpisode(show: Show): LocalEpisode | null {
  const started = show.episodes
    .filter((e) => e.video.resumeAt != null)
    .sort((a, b) => (b.video.lastPlayedAt ?? 0) - (a.video.lastPlayedAt ?? 0))[0];
  if (started) return started;
  let lastWatched = -1;
  show.episodes.forEach((e, i) => {
    if (e.video.watched) lastWatched = i;
  });
  return (
    show.episodes.slice(lastWatched + 1).find((e) => e.video.playable && !e.video.watched) ??
    show.episodes.find((e) => e.video.playable && !e.video.watched) ??
    null
  );
}

export function showSummary(show: Show) {
  const downloading = show.entries.filter((e) => ["resolving", "downloading", "starting"].includes(e.status)).length;
  const unwatched = show.episodes.filter((e) => e.video.playable && !e.video.watched).length;
  const watched = show.episodes.filter((e) => e.video.watched).length;
  return { downloading, unwatched, watched, total: show.episodes.length };
}

export function showKey(show: Show) {
  return show.tmdbId != null ? `tmdb:${show.tmdbId}` : `title:${show.title.toLowerCase()}`;
}

export function filmVideo(e: LibraryEntry): LocalEpisode | null {
  return e.videos[0] ? { entry: e, video: e.videos[0] } : null;
}

export function watchedShare(show: Show) {
  const s = showSummary(show);
  return s.total ? s.watched / s.total : 0;
}

export function playedShare(v: VideoEntry) {
  if (v.watched) return 1;
  return v.durationSecs ? v.positionSecs / v.durationSecs : 0;
}

export type UpNextItem = { ep: LocalEpisode; show: Show | null };

export interface UpNext {
  continueWatching: UpNextItem[];
  nextEpisodes: UpNextItem[];
  ready: UpNextItem[];
  recentlyAdded: Shelf[];
}

export function upNext(shelf: Shelf[]): UpNext {
  const continueWatching: UpNextItem[] = [];
  const nextEpisodes: UpNextItem[] = [];
  const ready: UpNextItem[] = [];
  for (const s of shelf) {
    if (s.kind === "film") {
      const ep = filmVideo(s.entry);
      if (!ep) continue;
      if (ep.video.resumeAt != null) continueWatching.push({ ep, show: null });
      else if (ep.video.playable && !ep.video.watched && ep.video.lastPlayedAt == null) ready.push({ ep, show: null });
      continue;
    }
    const ep = nextEpisode(s.show);
    if (!ep) continue;
    const started = s.show.episodes.some((e) => e.video.watched || e.video.lastPlayedAt != null);
    if (ep.video.resumeAt != null) continueWatching.push({ ep, show: s.show });
    else if (!ep.video.playable) continue;
    else if (started) nextEpisodes.push({ ep, show: s.show });
    else ready.push({ ep, show: s.show });
  }
  const played = (i: UpNextItem) => i.ep.video.lastPlayedAt ?? 0;
  continueWatching.sort((a, b) => played(b) - played(a));
  const added = (s: Shelf) => (s.kind === "film" ? s.entry.added_at : Math.max(...s.show.entries.map((e) => e.added_at)));
  const recentlyAdded = [...shelf].sort((a, b) => added(b) - added(a)).slice(0, 12);
  return { continueWatching, nextEpisodes, ready, recentlyAdded };
}
