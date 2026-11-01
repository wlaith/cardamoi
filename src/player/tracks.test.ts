// @vitest-environment jsdom
import { describe, expect, it } from "vitest";
import type { Track } from "../api";
import { trackName } from "./tracks";

const track = (over: Partial<Track>): Track => ({
  id: 1, kind: "sub", lang: "en", title: null, external: false, selected: false, forced: false, hearingImpaired: false, ...over,
});

describe("trackName", () => {
  it("is the language alone when there is nothing more to say", () => {
    expect(trackName(track({}))).toBe("English");
  });
  it("adds a title that says something", () => {
    expect(trackName(track({ title: "Commentary" }))).toBe("English · Commentary");
  });
  it("drops a title that is only the language or a file name", () => {
    expect(trackName(track({ title: "English" }))).toBe("English");
    expect(trackName(track({ title: "en.srt" }))).toBe("English");
    expect(trackName(track({ title: "Movie.2026.ass" }))).toBe("English");
  });
  it("copes with no language", () => {
    expect(trackName(track({ lang: null }))).toBe("Unknown language");
  });
});
