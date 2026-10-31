import { invoke } from "@tauri-apps/api/core";

export interface SearchResult {
  source: string;
  title: string;
  year: number | null;
  size_bytes: number | null;
  seeders: number | null;
  torrent_url: string;
  poster: string | null;
  details_url: string | null;
  single_video: boolean;
}

export interface Release {
  title: string;
  year: number | null;
  season: number | null;
  episode: number | null;
  resolution: string | null;
}

export interface Found extends SearchResult {
  release: Release;
  matches: boolean;
}

export interface SearchQuery {
  text: string;
  year: number | null;
  season: number | null;
  episode: number | null;
  match_title: string | null;
}

export type Kind = "movie" | "tv";

export interface CatalogTitle {
  id: number;
  kind: Kind;
  title: string;
  year: number | null;
  poster: string | null;
  backdrop: string | null;
  overview: string;
  rating: number | null;
}

export interface TmdbStatus {
  state: "missing" | "ok" | "rejected";
  kind: "token" | "key" | null;
  checkedAt: number | null;
  onboardingSeen: boolean;
}

export interface Season {
  number: number;
  name: string;
  episodeCount: number;
  poster: string | null;
}

export interface Details extends CatalogTitle {
  runtime: number | null;
  genres: string[];
  seasons: Season[];
}

export interface NameLookup {
  query: string;
  season: number | null;
  episode: number | null;
  found: CatalogTitle | null;
}

export interface Episode {
  number: number;
  name: string;
  overview: string;
  airDate: string | null;
  still: string | null;
}

export type Status = "resolving" | "downloading" | "paused" | "done" | "starting" | "error" | "missing";

export interface LibraryEntry {
  id: number;
  title: string;
  year: number | null;
  source: string;
  info_hash: string;
  poster: string | null;
  added_at: number;
  tmdb_id: number | null;
  kind: Kind | null;
  season: number | null;
  episode: number | null;
  torrent_url: string | null;
  single_video: boolean;
  status: Status;
  progress: number;
  downloadedBytes: number;
  totalBytes: number;
  speedMibps: number;
  eta: string | null;
  error: string | null;
  resolvingSecs: number | null;
  videos: VideoEntry[];
}

export interface VideoEntry {
  itemId: number;
  fileIndex: number;
  path: string;
  len: number;
  season: number | null;
  episode: number | null;
  positionSecs: number;
  durationSecs: number | null;
  watched: boolean;
  lastPlayedAt: number | null;
  progress: number;
  playable: boolean;
  readyInSecs: number | null;
  resumeAt: number | null;
}

export interface Track {
  id: number;
  kind: "video" | "audio" | "sub";
  lang: string | null;
  title: string | null;
  external: boolean;
  selected: boolean;
  forced: boolean;
  hearingImpaired: boolean;
}

export interface PlayerState {
  itemId: number;
  fileIndex: number;
  title: string;
  status: "opening" | "playing" | "ended" | "stopped" | "failed" | "crashed";
  position: number;
  duration: number | null;
  paused: boolean;
  buffering: boolean;
  watched: boolean;
  tracks: Track[];
  subDelay: number;
  error: string | null;
}

export interface SubtitleSettings {
  languages: string[];
  autoFetch: boolean;
  opensubtitlesKey: string;
  opensubtitlesUser: string;
  opensubtitlesPassword?: string;
  hasOpensubtitlesPassword: boolean;
  subdlKey: string;
}

export interface Subtitle {
  id: number;
  itemId: number;
  fileIndex: number | null;
  lang: string | null;
  label: string;
  path: string;
  source: "torrent" | "opensubtitles" | "subdl";
  hearingImpaired: boolean;
  providerKey: string | null;
}

export interface FoundSubtitle {
  provider: "opensubtitles" | "subdl";
  key: string;
  lang: string;
  release: string;
  hearingImpaired: boolean;
  hashMatch: boolean;
  downloads: number | null;
}

export type Confidence = "file" | "release" | "maybe";

export interface RankedSubtitle extends FoundSubtitle {
  confidence: Confidence;
}

export interface Quota {
  remaining: number;
  resetsAt: string | null;
}

export type PlayError =
  | { kind: "mpvMissing" }
  | { kind: "fileMissing" }
  | { kind: "notReady" }
  | { kind: "other"; message: string };

export interface AddRequest {
  title: string;
  year: number | null;
  source: string;
  torrentUrl: string;
  poster: string | null;
  singleVideo: boolean;
  tmdbId?: number | null;
  kind?: Kind | null;
  season?: number | null;
  episode?: number | null;
}

export interface SourceRow {
  id: number;
  name: string;
  kind: string;
  url: string;
  api_key: string;
  enabled: boolean;
}

export type ProwlarrStatus =
  | { state: "starting" }
  | { state: "ready" }
  | { state: "failed"; message: string };

export const api = {
  search: (query: SearchQuery) => invoke<{ results: Found[]; errors: string[] }>("search", { query }),
  catalogSearch: (text: string) => invoke<CatalogTitle[]>("catalog_search", { text }),
  matchName: (name: string) => invoke<NameLookup>("match_name", { name }),
  titleDetails: (kind: Kind, id: number) => invoke<Details>("title_details", { kind, id }),
  seasonEpisodes: (id: number, season: number) => invoke<Episode[]>("season_episodes", { id, season }),
  tmdbStatus: () => invoke<TmdbStatus>("tmdb_status"),
  saveTmdbKey: (key: string, skipCheck = false) => invoke<number>("save_tmdb_key", { key, skipCheck }),
  removeTmdbKey: () => invoke<void>("remove_tmdb_key"),
  markOnboardingSeen: () => invoke<void>("mark_onboarding_seen"),
  prowlarrStatus: () => invoke<ProwlarrStatus>("prowlarr_status"),
  openProwlarr: () => invoke<void>("open_prowlarr"),
  listSources: () => invoke<SourceRow[]>("list_sources"),
  addSource: (name: string, url: string, apiKey: string) =>
    invoke<void>("add_source", { name, url, apiKey }),
  setSourceEnabled: (id: number, enabled: boolean) => invoke<void>("set_source_enabled", { id, enabled }),
  removeSource: (id: number) => invoke<void>("remove_source", { id }),
  addDownload: (req: AddRequest) => invoke<void>("add_download", { req }),
  retryDownload: (id: number) => invoke<void>("retry_download", { id }),
  scanLibraryFolder: () => invoke<number>("scan_library_folder"),
  takePendingMagnet: () => invoke<string | null>("take_pending_magnet"),
  magnetHandlerIsUs: () => invoke<boolean>("magnet_handler_is_us"),
  makeMagnetHandler: () => invoke<boolean>("make_magnet_handler"),
  openLibraryFolder: () => invoke<string>("open_library_folder"),
  library: () => invoke<LibraryEntry[]>("library"),
  pause: (id: number) => invoke<void>("pause", { id }),
  resume: (id: number) => invoke<void>("resume", { id }),
  remove: (id: number) => invoke<void>("remove", { id }),
  play: (id: number, fileIndex: number | null = null, fromStart = false) =>
    invoke<void>("play", { id, fileIndex, fromStart }),
  openExternal: (id: number, fileIndex: number | null = null) => invoke<void>("open_external", { id, fileIndex }),
  playerState: () => invoke<PlayerState | null>("player_state"),
  playerCommand: (args: (string | number)[]) => invoke<void>("player_command", { args }),
  stopPlayer: () => invoke<void>("stop_player"),
  markWatched: (id: number, fileIndex: number, watched: boolean) =>
    invoke<void>("mark_watched", { id, fileIndex, watched }),
  markSeasonWatched: (tmdbId: number, season: number, watched: boolean) =>
    invoke<void>("mark_season_watched", { tmdbId, season, watched }),
  playbackSettings: () => invoke<{ autoplayNext: boolean }>("playback_settings"),
  savePlaybackSettings: (settings: { autoplayNext: boolean }) => invoke<void>("save_playback_settings", { settings }),
  subtitleSettings: () => invoke<SubtitleSettings>("subtitle_settings"),
  saveSubtitleSettings: (settings: SubtitleSettings) => invoke<void>("save_subtitle_settings", { settings }),
  listSubtitles: (id: number, fileIndex: number) => invoke<Subtitle[]>("list_subtitles", { id, fileIndex }),
  searchSubtitles: (id: number, fileIndex: number, languages: string[]) =>
    invoke<{ results: RankedSubtitle[]; errors: string[]; quota: Quota | null }>("search_subtitles", {
      id,
      fileIndex,
      languages,
    }),
  downloadSubtitle: (id: number, fileIndex: number, found: FoundSubtitle) =>
    invoke<Subtitle>("download_subtitle", { id, fileIndex, found }),
  reveal: (id: number) => invoke<void>("reveal", { id }),
};

export function errorMessage(e: unknown): string {
  return plainError(e).message;
}

const KNOWN_FAILURES: [RegExp, string][] = [
  [/no space left|disk full/i, "The disk is full. Free up some space and try again."],
  [/tmdb:missing/, "Add a TMDB key in Preferences › TMDB to look up films and shows."],
  [/tmdb:rejected/, "TMDB turned down your key. Update it in Preferences › TMDB."],
  [/tmdb:format/, "This doesn't look like a TMDB key. Copy the API Read Access Token from your TMDB API settings."],
  [/tmdb:unreachable/, "Couldn't reach TMDB to check the key. Check that you're online."],
  [/\b(401|403)\b|unauthori[sz]ed|forbidden|invalid api ?key/i, "The API key was turned down. Check it in Preferences."],
  [/\b429\b|too many requests|quota/i, "The service is busy or its daily limit is used up. Try again later."],
  [
    /connection refused|error sending request|dns error|timed out|failed to lookup|network is unreachable/i,
    "Couldn't reach the server. Check that it's running and that you're online.",
  ],
];

export function plainError(e: unknown): { message: string; details: string } {
  if (e && typeof e === "object" && "kind" in e) {
    const err = e as PlayError;
    switch (err.kind) {
      case "mpvMissing":
        return {
          message: "cardamoi plays videos with mpv, which isn't installed. Install it with Homebrew (brew install mpv), or open this video in your system player.",
          details: "",
        };
      case "fileMissing":
        return { message: "The video file is missing from the downloads folder.", details: "" };
      case "notReady":
        return { message: "Not enough of this video has downloaded to start yet.", details: "" };
      case "other":
        return plainError(err.message);
    }
  }
  const raw = String(e).replace(/^Error:\s*/, "");
  const known = KNOWN_FAILURES.find(([pattern]) => pattern.test(raw));
  if (known) return { message: known[1], details: raw };
  // Chains read "tried: failed: cause"; only the first part is shown.
  const cut = raw.indexOf(": ");
  if (cut > 0 && raw.length > 70) return { message: `${raw.slice(0, cut)}.`, details: raw };
  return { message: raw, details: "" };
}

export function isMpvMissing(e: unknown): boolean {
  return !!e && typeof e === "object" && "kind" in e && (e as PlayError).kind === "mpvMissing";
}

export const SUBTITLE_LANGUAGES = [
  "en", "ar", "fr", "es", "de", "it", "pt", "ru", "tr", "nl",
  "zh", "ja", "ko", "fa", "he", "hi", "id", "pl", "sv", "el",
];

export function languageName(code: string | null): string {
  if (!code) return "Unknown language";
  try {
    return new Intl.DisplayNames([navigator.language], { type: "language" }).of(code) ?? code;
  } catch {
    return code;
  }
}

export function systemLanguage(): string {
  const code = navigator.language.split("-")[0].toLowerCase();
  return SUBTITLE_LANGUAGES.includes(code) ? code : "en";
}

export function formatBytes(bytes: number | null): string {
  if (!bytes) return "";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let n = bytes;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  return `${n.toFixed(n >= 10 || i === 0 ? 0 : 1)} ${units[i]}`;
}

export function magnetName(magnet: string): string {
  const match = magnet.match(/[?&]dn=([^&]+)/);
  if (!match) return "";
  try {
    return decodeURIComponent(match[1].replace(/\+/g, " "));
  } catch {
    return match[1];
  }
}

export function timeLeft(v: VideoEntry): string {
  if (v.durationSecs == null) return "";
  const minutes = Math.max(1, Math.round((v.durationSecs - v.positionSecs) / 60));
  return minutes >= 60 ? `${Math.floor(minutes / 60)} h ${minutes % 60} min left` : `${minutes} min left`;
}

export function episodeLabel(season: number | null, episode: number | null): string {
  if (season == null) return "";
  const s = String(season).padStart(2, "0");
  return episode == null ? `Season ${season}` : `S${s}E${String(episode).padStart(2, "0")}`;
}
