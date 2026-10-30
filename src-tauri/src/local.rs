use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;

use crate::release::{self, Release};

pub const MIN_VIDEO_BYTES: u64 = 20 * 1024 * 1024;
const MAX_DEPTH: usize = 5;

static EPISODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(?:episode|ep|e)\s*\.?\s*(\d{1,3})\b|^(\d{1,3})\s*[-.]\s").unwrap());

pub fn find_videos(dir: &Path, is_video: impl Fn(&Path) -> bool + Copy) -> Vec<(PathBuf, u64)> {
    let mut found = vec![];
    walk(dir, 0, is_video, &mut found);
    found
}

fn walk(dir: &Path, depth: usize, is_video: impl Fn(&Path) -> bool + Copy, found: &mut Vec<(PathBuf, u64)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_dir() && depth < MAX_DEPTH {
            walk(&path, depth + 1, is_video, found);
        } else if meta.is_file() && is_video(&path) {
            found.push((path, meta.len()));
        }
    }
}

pub fn describe(path: &Path) -> Release {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let mut own = release::parse(&stem);
    let folder = path.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    if let Some(folder) = folder {
        let theirs = release::parse(&folder);
        if own.title.is_empty() || EPISODE.is_match(&own.title) || (own.season.is_none() && theirs.season.is_some()) {
            own.title = theirs.title;
            own.year = own.year.or(theirs.year);
            own.season = own.season.or(theirs.season);
        }
    }
    if own.season.is_some() && own.episode.is_none() {
        own.episode = EPISODE
            .captures(&stem.replace(['.', '_'], " "))
            .and_then(|c| c.get(1).or(c.get(2)))
            .and_then(|m| m.as_str().parse().ok());
    }
    own
}

#[cfg(test)]
mod tests {
    use super::*;

    fn describe_str(path: &str) -> (String, Option<i32>, Option<u32>, Option<u32>) {
        let r = describe(Path::new(path));
        (r.title, r.year, r.season, r.episode)
    }

    #[test]
    fn names_that_say_it_all() {
        assert_eq!(describe_str("/m/Dune.2021.1080p.mkv"), ("Dune".into(), Some(2021), None, None));
        assert_eq!(describe_str("/m/Severance.S01E03.mkv"), ("Severance".into(), None, Some(1), Some(3)));
    }

    #[test]
    fn the_folder_fills_in_the_show() {
        assert_eq!(
            describe_str("/m/Severance Season 1/Episode 3.mkv"),
            ("Severance".into(), None, Some(1), Some(3))
        );
        assert_eq!(
            describe_str("/m/Severance (2022) S01/03 - In Perpetuity.mkv"),
            ("Severance".into(), Some(2022), Some(1), Some(3))
        );
        assert_eq!(describe_str("/m/The Bear S02/E05.mkv"), ("The Bear".into(), None, Some(2), Some(5)));
    }

    #[test]
    fn a_film_in_its_own_folder_keeps_its_name() {
        assert_eq!(describe_str("/m/Dune (2021)/Dune.2021.mkv"), ("Dune".into(), Some(2021), None, None));
    }

    #[test]
    fn finds_videos_and_skips_hidden_folders() {
        let dir = std::env::temp_dir().join(format!("cardamoi-local-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Show S01")).unwrap();
        std::fs::create_dir_all(dir.join(".Trash")).unwrap();
        std::fs::write(dir.join("Show S01/E01.mkv"), b"x").unwrap();
        std::fs::write(dir.join(".Trash/old.mkv"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let is_video = |p: &Path| p.extension().is_some_and(|e| e == "mkv");
        let found = find_videos(&dir, is_video);
        assert_eq!(found, [(dir.join("Show S01/E01.mkv"), 1)]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
