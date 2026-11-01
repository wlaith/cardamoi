import { languageName, type Track } from "../api";

const FILE_NAME = /\.(srt|ass|ssa|sub|vtt|idx)$/i;

/** "English", or "English · Commentary" when the title adds something: not the language again, not a file name. */
export function trackName(t: Track): string {
  const language = languageName(t.lang);
  const title = t.title?.trim();
  return title && title !== language && !FILE_NAME.test(title) ? `${language} · ${title}` : language;
}
