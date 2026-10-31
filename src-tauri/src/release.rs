use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Release {
    pub title: String,
    pub year: Option<i32>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub resolution: Option<String>,
}

static EPISODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bS(\d{1,2}) ?E(\d{1,3})\b|\b(\d{1,2})x(\d{2,3})\b").unwrap());
static SEASON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bS(\d{1,2})\b|\bseason (\d{1,2})\b").unwrap());
static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[(\[]?\b(19\d{2}|20\d{2})\b[)\]]?").unwrap());
static RESOLUTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(2160p|4k|1080p|720p|576p|480p)\b").unwrap());
static JUNK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(\d{3,4}p|4k|uhd|hdr|bluray|blu-ray|brrip|bdrip|web-?dl|webrip|web|hdtv|dvdrip|dvd|remux|x26[45]|h\.?26[45]|hevc|av1|svt-av1|\d{1,2}-?bit|xvid|aac|ac3|dts|complete|proper|repack|extended|unrated|multi)\b",
    )
    .unwrap()
});

pub fn parse(name: &str) -> Release {
    let name = name.replace(['.', '_'], " ");
    let name = match name.trim_start().strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        Some((_, rest)) => rest.to_owned(),
        None => name,
    };
    let mut cut = name.len();
    if let Some(i) = name.find('[').filter(|&i| i > 0) {
        cut = i;
    }
    let mut release = Release::default();

    if let Some(c) = EPISODE.captures(&name) {
        let (s, e) = match (c.get(1), c.get(2)) {
            (Some(s), Some(e)) => (s, e),
            _ => (c.get(3).unwrap(), c.get(4).unwrap()),
        };
        release.season = s.as_str().parse().ok();
        release.episode = e.as_str().parse().ok();
        cut = cut.min(c.get(0).unwrap().start());
    } else if let Some(c) = SEASON.captures(&name) {
        release.season = c.get(1).or(c.get(2)).and_then(|m| m.as_str().parse().ok());
        cut = cut.min(c.get(0).unwrap().start());
    }

    // A leading year belongs to the title ("2001 A Space Odyssey").
    if let Some(c) = YEAR.captures_iter(&name).find(|c| c.get(0).unwrap().start() > 0) {
        release.year = c[1].parse().ok();
        cut = cut.min(c.get(0).unwrap().start());
    }
    if let Some(m) = JUNK.find(&name) {
        cut = cut.min(m.start());
    }
    release.resolution = RESOLUTION.find(&name).map(|m| m.as_str().to_lowercase().replace("4k", "2160p"));
    release.title = name[..cut].trim_matches(|c: char| c.is_whitespace() || "-([".contains(c)).to_owned();
    release
}

pub fn normalize(title: &str) -> String {
    let lower = title.to_lowercase().replace('&', " and ");
    let words: Vec<String> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect();
    let words = match words.first() {
        Some(w) if w == "the" && words.len() > 1 => &words[1..],
        _ => &words[..],
    };
    words.join(" ")
}

pub fn matches(release: &Release, title: &str, year: Option<i32>, season: Option<u32>, episode: Option<u32>) -> bool {
    if normalize(&release.title) != normalize(title) {
        return false;
    }
    if let (Some(want), Some(got)) = (year, release.year) {
        // Release years are often off by one.
        if season.is_none() && (want - got).abs() > 1 {
            return false;
        }
    }
    match (season, episode) {
        (Some(s), Some(e)) => release.season == Some(s) && release.episode.is_none_or(|got| got == e),
        (Some(s), None) => release.season == Some(s) && release.episode.is_none(),
        _ => release.season.is_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn movie() {
        let r = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
        assert_eq!(r.title, "The Matrix");
        assert_eq!(r.year, Some(1999));
        assert_eq!(r.resolution.as_deref(), Some("1080p"));
        assert_eq!(r.season, None);
    }

    #[test]
    fn movie_with_parenthesised_year_and_number_title() {
        let r = parse("2001 A Space Odyssey (1968) 2160p UHD");
        assert_eq!(r.title, "2001 A Space Odyssey");
        assert_eq!(r.year, Some(1968));
        assert_eq!(r.resolution.as_deref(), Some("2160p"));
    }

    #[test]
    fn episode() {
        let r = parse("Severance.S02E03.720p.WEB.h264-GRP");
        assert_eq!(r.title, "Severance");
        assert_eq!((r.season, r.episode), (Some(2), Some(3)));
        assert_eq!(r.resolution.as_deref(), Some("720p"));
    }

    #[test]
    fn episode_alt_format_and_season_pack() {
        let r = parse("Les Sopranos 1x01 Egarement");
        assert_eq!(r.title, "Les Sopranos");
        assert_eq!((r.season, r.episode), (Some(1), Some(1)));

        let r = parse("The Wire Season 3 Complete 1080p");
        assert_eq!(r.title, "The Wire");
        assert_eq!((r.season, r.episode), (Some(3), None));

        let r = parse("Breaking_Bad_S05_4K");
        assert_eq!(r.title, "Breaking Bad");
        assert_eq!((r.season, r.episode), (Some(5), None));
        assert_eq!(r.resolution.as_deref(), Some("2160p"));
    }

    #[test]
    fn bracketed_alternate_title_and_group_tag() {
        let r = parse("La bola negra [The Black Ball] 10bit 4KDS 1080p SVT-AV1-Tritium.mkv");
        assert_eq!(r.title, "La bola negra");
        assert_eq!(r.resolution.as_deref(), Some("1080p"));

        let r = parse("[SubsPlease] Frieren - S01E05 (1080p) [ABCD1234].mkv");
        assert_eq!(r.title, "Frieren");
        assert_eq!((r.season, r.episode), (Some(1), Some(5)));

        let r = parse("Dune Part Two 2024 10bit 2160p");
        assert_eq!(r.title, "Dune Part Two");
    }

    #[test]
    fn matching() {
        let movie = parse("The.Matrix.1999.1080p.BluRay");
        assert!(matches(&movie, "The Matrix", Some(1999), None, None));
        assert!(matches(&movie, "Matrix", Some(2000), None, None));
        assert!(!matches(&movie, "The Matrix Reloaded", Some(2003), None, None));
        assert!(!matches(&movie, "The Matrix", Some(2021), None, None));

        let ep = parse("Severance.S02E03.720p");
        let pack = parse("Severance.S02.1080p");
        assert!(matches(&ep, "Severance", Some(2022), Some(2), Some(3)));
        assert!(!matches(&ep, "Severance", None, Some(2), Some(4)));
        assert!(matches(&pack, "Severance", None, Some(2), Some(3)));
        assert!(matches(&pack, "Severance", None, Some(2), None));
        assert!(!matches(&ep, "Severance", None, Some(2), None));
        assert!(!matches(&ep, "Severance", None, None, None));
    }

    #[test]
    fn normalizing() {
        assert_eq!(normalize("The Lord of the Rings: The Two Towers"), "lord of the rings the two towers");
        assert_eq!(normalize("Law & Order"), normalize("Law and Order"));
    }
}
