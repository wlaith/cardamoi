use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub source: String,
    pub info_hash: String,
    pub poster: Option<String>,
    pub added_at: i64,
    pub tmdb_id: Option<i64>,
    pub kind: Option<String>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub torrent_url: Option<String>,
    pub single_video: bool,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub item_id: i64,
    pub file_index: i64,
    pub path: String,
    pub len: u64,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub position_secs: f64,
    pub duration_secs: Option<f64>,
    pub watched: bool,
    pub last_played_at: Option<i64>,
}

pub struct NewVideo {
    pub file_index: i64,
    pub path: String,
    pub len: u64,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

pub struct NewItem<'a> {
    pub title: &'a str,
    pub year: Option<i32>,
    pub source: &'a str,
    pub info_hash: &'a str,
    pub poster: Option<&'a str>,
    pub tmdb_id: Option<i64>,
    pub kind: Option<&'a str>,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub torrent_url: Option<&'a str>,
    pub single_video: bool,
    pub local_path: Option<&'a str>,
}

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS items (
            id         INTEGER PRIMARY KEY,
            title      TEXT NOT NULL,
            year       INTEGER,
            source     TEXT NOT NULL,
            info_hash  TEXT NOT NULL UNIQUE,
            poster     TEXT,
            added_at   INTEGER NOT NULL DEFAULT (unixepoch())
        );
        CREATE TABLE IF NOT EXISTS sources (
            id       INTEGER PRIMARY KEY,
            name     TEXT NOT NULL,
            kind     TEXT NOT NULL DEFAULT 'torznab',
            url      TEXT NOT NULL,
            api_key  TEXT NOT NULL DEFAULT '',
            enabled  INTEGER NOT NULL DEFAULT 1
        );
        CREATE TABLE IF NOT EXISTS videos (
            item_id     INTEGER NOT NULL,
            file_index  INTEGER NOT NULL,
            path        TEXT NOT NULL,
            len         INTEGER NOT NULL,
            season      INTEGER,
            episode     INTEGER,
            PRIMARY KEY (item_id, file_index)
        );
        CREATE TABLE IF NOT EXISTS playback (
            item_id        INTEGER NOT NULL,
            -- -1 until the download's videos are known: its main video.
            file_index     INTEGER NOT NULL DEFAULT -1,
            position_secs  REAL NOT NULL DEFAULT 0,
            duration_secs  REAL,
            watched        INTEGER NOT NULL DEFAULT 0,
            sub_lang       TEXT,
            audio_lang     TEXT,
            sub_offset_ms  INTEGER NOT NULL DEFAULT 0,
            updated_at     INTEGER NOT NULL DEFAULT (unixepoch()),
            PRIMARY KEY (item_id, file_index)
        );
        -- Track choices carried from one episode of a show to the next.
        CREATE TABLE IF NOT EXISTS show_prefs (
            tmdb_id        INTEGER PRIMARY KEY,
            sub_lang       TEXT,
            audio_lang     TEXT,
            sub_offset_ms  INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS settings (
            key    TEXT PRIMARY KEY,
            value  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS subtitles (
            id                INTEGER PRIMARY KEY,
            item_id           INTEGER NOT NULL,
            lang              TEXT,
            label             TEXT NOT NULL,
            path              TEXT NOT NULL,
            -- torrent, opensubtitles or subdl
            source            TEXT NOT NULL,
            hearing_impaired  INTEGER NOT NULL DEFAULT 0,
            -- The provider's id for it, so search results can show it is already here.
            provider_key      TEXT,
            UNIQUE (item_id, path)
        );",
    )?;
    for (table, column, ty) in [
        ("items", "tmdb_id", "INTEGER"),
        ("items", "kind", "TEXT"),
        ("items", "season", "INTEGER"),
        ("items", "episode", "INTEGER"),
        ("items", "torrent_url", "TEXT"),
        ("items", "single_video", "INTEGER NOT NULL DEFAULT 0"),
        ("items", "local_path", "TEXT"),
        ("subtitles", "file_index", "INTEGER"),
    ] {
        if !has_column(&conn, table, column)? {
            conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {ty}"), [])?;
        }
    }
    // Move playback from per-download to per-episode.
    if !has_column(&conn, "playback", "file_index")? {
        conn.execute_batch(
            "ALTER TABLE playback RENAME TO playback_old;
             CREATE TABLE playback (
                 item_id        INTEGER NOT NULL,
                 file_index     INTEGER NOT NULL DEFAULT -1,
                 position_secs  REAL NOT NULL DEFAULT 0,
                 duration_secs  REAL,
                 watched        INTEGER NOT NULL DEFAULT 0,
                 sub_lang       TEXT,
                 audio_lang     TEXT,
                 sub_offset_ms  INTEGER NOT NULL DEFAULT 0,
                 updated_at     INTEGER NOT NULL DEFAULT (unixepoch()),
                 PRIMARY KEY (item_id, file_index)
             );
             INSERT INTO playback (item_id, position_secs, duration_secs, watched, sub_lang, audio_lang, sub_offset_ms, updated_at)
                 SELECT item_id, position_secs, duration_secs, watched, sub_lang, audio_lang, sub_offset_ms, updated_at
                 FROM playback_old;
             DROP TABLE playback_old;",
        )?;
    }
    conn.execute("DROP TABLE IF EXISTS flaresolverr", [])?;
    Ok(conn)
}

pub fn insert(conn: &Connection, item: &NewItem) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO items (title, year, source, info_hash, poster, tmdb_id, kind, season, episode, torrent_url,
                            single_video, local_path)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(info_hash) DO NOTHING",
        params![
            item.title,
            item.year,
            item.source,
            item.info_hash,
            item.poster,
            item.tmdb_id,
            item.kind,
            item.season,
            item.episode,
            item.torrent_url,
            item.single_video,
            item.local_path
        ],
    )?;
    conn.query_row("SELECT id FROM items WHERE info_hash = ?1", params![item.info_hash], |r| r.get(0))
}

pub struct Match<'a> {
    pub title: &'a str,
    pub year: Option<i32>,
    pub poster: Option<&'a str>,
    pub tmdb_id: i64,
    pub kind: &'a str,
    pub season: Option<u32>,
    pub episode: Option<u32>,
}

pub fn set_match(conn: &Connection, id: i64, m: &Match) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE items SET title = ?2, year = ?3, poster = COALESCE(?4, poster), tmdb_id = ?5, kind = ?6,
             season = ?7, episode = ?8
         WHERE id = ?1",
        params![id, m.title, m.year, m.poster, m.tmdb_id, m.kind, m.season, m.episode],
    )?;
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
        params![table, column],
        |r| r.get(0),
    )
}

const SELECT: &str = "SELECT id, title, year, source, info_hash, poster, added_at, tmdb_id, kind, season, episode,
            torrent_url, single_video, local_path
     FROM items";

fn row_to_item(r: &rusqlite::Row) -> rusqlite::Result<Item> {
    Ok(Item {
        id: r.get(0)?,
        title: r.get(1)?,
        year: r.get(2)?,
        source: r.get(3)?,
        info_hash: r.get(4)?,
        poster: r.get(5)?,
        added_at: r.get(6)?,
        tmdb_id: r.get(7)?,
        kind: r.get(8)?,
        season: r.get(9)?,
        episode: r.get(10)?,
        torrent_url: r.get(11)?,
        single_video: r.get(12)?,
        local_path: r.get(13)?,
    })
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Item>> {
    let mut stmt = conn.prepare(&format!("{SELECT} ORDER BY added_at DESC, id DESC"))?;
    let rows = stmt.query_map([], row_to_item)?;
    rows.collect()
}

pub fn get(conn: &Connection, id: i64) -> rusqlite::Result<Item> {
    conn.query_row(&format!("{SELECT} WHERE id = ?1"), params![id], row_to_item)
}

pub fn delete(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM items WHERE id = ?1", params![id])?;
    conn.execute("DELETE FROM videos WHERE item_id = ?1", params![id])?;
    conn.execute("DELETE FROM playback WHERE item_id = ?1", params![id])?;
    conn.execute("DELETE FROM subtitles WHERE item_id = ?1", params![id])?;
    Ok(())
}

pub fn save_videos(conn: &Connection, item_id: i64, videos: &[NewVideo], main: i64) -> rusqlite::Result<()> {
    for v in videos {
        conn.execute(
            "INSERT INTO videos (item_id, file_index, path, len, season, episode) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(item_id, file_index) DO NOTHING",
            params![item_id, v.file_index, v.path, v.len as i64, v.season, v.episode],
        )?;
    }
    conn.execute(
        "UPDATE OR IGNORE playback SET file_index = ?2 WHERE item_id = ?1 AND file_index = -1",
        params![item_id, main],
    )?;
    conn.execute(
        "UPDATE subtitles SET file_index = ?2 WHERE item_id = ?1 AND file_index IS NULL",
        params![item_id, main],
    )?;
    Ok(())
}

const VIDEO_SELECT: &str = "SELECT v.item_id, v.file_index, v.path, v.len, v.season, v.episode,
            COALESCE(p.position_secs, 0), p.duration_secs, COALESCE(p.watched, 0), p.updated_at
     FROM videos v LEFT JOIN playback p ON p.item_id = v.item_id AND p.file_index = v.file_index";

fn row_to_video(r: &rusqlite::Row) -> rusqlite::Result<Video> {
    Ok(Video {
        item_id: r.get(0)?,
        file_index: r.get(1)?,
        path: r.get(2)?,
        len: r.get::<_, i64>(3)? as u64,
        season: r.get(4)?,
        episode: r.get(5)?,
        position_secs: r.get(6)?,
        duration_secs: r.get(7)?,
        watched: r.get(8)?,
        last_played_at: r.get(9)?,
    })
}

pub fn videos_for(conn: &Connection, item_id: i64) -> rusqlite::Result<Vec<Video>> {
    let mut stmt =
        conn.prepare(&format!("{VIDEO_SELECT} WHERE v.item_id = ?1 ORDER BY v.season, v.episode, v.file_index"))?;
    let rows = stmt.query_map(params![item_id], row_to_video)?;
    rows.collect()
}

pub fn set_season_watched(conn: &Connection, tmdb_id: i64, season: u32, watched: bool) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO playback (item_id, file_index, watched)
             SELECT v.item_id, v.file_index, ?3 FROM videos v JOIN items i ON i.id = v.item_id
             WHERE i.tmdb_id = ?1 AND i.kind = 'tv' AND v.season = ?2
         ON CONFLICT(item_id, file_index) DO UPDATE SET watched = excluded.watched, updated_at = unixepoch()",
        params![tmdb_id, season, watched],
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subtitle {
    pub id: i64,
    pub item_id: i64,
    pub lang: Option<String>,
    pub label: String,
    pub path: String,
    pub source: String,
    pub hearing_impaired: bool,
    pub provider_key: Option<String>,
    pub file_index: Option<i64>,
}

pub struct NewSubtitle<'a> {
    pub item_id: i64,
    pub file_index: i64,
    pub lang: Option<&'a str>,
    pub label: &'a str,
    pub path: &'a str,
    pub source: &'a str,
    pub hearing_impaired: bool,
    pub provider_key: Option<&'a str>,
}

pub fn insert_subtitle(conn: &Connection, sub: &NewSubtitle) -> rusqlite::Result<Subtitle> {
    conn.execute(
        "INSERT INTO subtitles (item_id, lang, label, path, source, hearing_impaired, provider_key, file_index)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(item_id, path) DO NOTHING",
        params![
            sub.item_id,
            sub.lang,
            sub.label,
            sub.path,
            sub.source,
            sub.hearing_impaired,
            sub.provider_key,
            sub.file_index
        ],
    )?;
    conn.query_row(
        &format!("{SUBTITLE_SELECT} WHERE item_id = ?1 AND path = ?2"),
        params![sub.item_id, sub.path],
        row_to_subtitle,
    )
}

const SUBTITLE_SELECT: &str =
    "SELECT id, item_id, lang, label, path, source, hearing_impaired, provider_key, file_index FROM subtitles";

fn row_to_subtitle(r: &rusqlite::Row) -> rusqlite::Result<Subtitle> {
    Ok(Subtitle {
        id: r.get(0)?,
        item_id: r.get(1)?,
        lang: r.get(2)?,
        label: r.get(3)?,
        path: r.get(4)?,
        source: r.get(5)?,
        hearing_impaired: r.get(6)?,
        provider_key: r.get(7)?,
        file_index: r.get(8)?,
    })
}

pub fn list_subtitles(conn: &Connection, item_id: i64, file_index: i64) -> rusqlite::Result<Vec<Subtitle>> {
    let mut stmt = conn.prepare(&format!(
        "{SUBTITLE_SELECT} WHERE item_id = ?1 AND (file_index = ?2 OR file_index IS NULL) ORDER BY id"
    ))?;
    let rows = stmt.query_map(params![item_id, file_index], row_to_subtitle)?;
    rows.collect()
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackPrefs {
    pub sub_lang: Option<String>,
    pub audio_lang: Option<String>,
    pub sub_offset_ms: i64,
}

fn row_to_tracks(r: &rusqlite::Row) -> rusqlite::Result<TrackPrefs> {
    Ok(TrackPrefs { sub_lang: r.get(0)?, audio_lang: r.get(1)?, sub_offset_ms: r.get(2)? })
}

pub fn get_tracks(conn: &Connection, item_id: i64, file_index: i64) -> rusqlite::Result<Option<TrackPrefs>> {
    conn.query_row(
        "SELECT sub_lang, audio_lang, sub_offset_ms FROM playback WHERE item_id = ?1 AND file_index = ?2",
        params![item_id, file_index],
        row_to_tracks,
    )
    .optional()
}

pub fn save_position(
    conn: &Connection,
    item_id: i64,
    file_index: i64,
    position: f64,
    duration: Option<f64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO playback (item_id, file_index, position_secs, duration_secs) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(item_id, file_index) DO UPDATE SET
             position_secs = excluded.position_secs,
             duration_secs = COALESCE(excluded.duration_secs, duration_secs),
             updated_at = unixepoch()",
        params![item_id, file_index, position, duration],
    )?;
    Ok(())
}

pub fn set_watched(conn: &Connection, item_id: i64, file_index: i64, watched: bool) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO playback (item_id, file_index, watched) VALUES (?1, ?2, ?3)
         ON CONFLICT(item_id, file_index) DO UPDATE SET watched = excluded.watched, updated_at = unixepoch()",
        params![item_id, file_index, watched],
    )?;
    Ok(())
}

pub fn save_tracks(conn: &Connection, item_id: i64, file_index: i64, tracks: &TrackPrefs) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO playback (item_id, file_index, sub_lang, audio_lang, sub_offset_ms) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(item_id, file_index) DO UPDATE SET
             sub_lang = excluded.sub_lang,
             audio_lang = excluded.audio_lang,
             sub_offset_ms = excluded.sub_offset_ms",
        params![item_id, file_index, tracks.sub_lang, tracks.audio_lang, tracks.sub_offset_ms],
    )?;
    Ok(())
}

pub fn get_show_tracks(conn: &Connection, tmdb_id: i64) -> rusqlite::Result<Option<TrackPrefs>> {
    conn.query_row(
        "SELECT sub_lang, audio_lang, sub_offset_ms FROM show_prefs WHERE tmdb_id = ?1",
        params![tmdb_id],
        row_to_tracks,
    )
    .optional()
}

pub fn save_show_tracks(conn: &Connection, tmdb_id: i64, tracks: &TrackPrefs) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO show_prefs (tmdb_id, sub_lang, audio_lang, sub_offset_ms) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(tmdb_id) DO UPDATE SET
             sub_lang = excluded.sub_lang,
             audio_lang = excluded.audio_lang,
             sub_offset_ms = excluded.sub_offset_ms",
        params![tmdb_id, tracks.sub_lang, tracks.audio_lang, tracks.sub_offset_ms],
    )?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| r.get(0))
        .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRow {
    pub id: i64,
    pub name: String,
    pub kind: String,
    pub url: String,
    pub api_key: String,
    pub enabled: bool,
}

pub fn list_sources(conn: &Connection) -> rusqlite::Result<Vec<SourceRow>> {
    let mut stmt = conn.prepare("SELECT id, name, kind, url, api_key, enabled FROM sources ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok(SourceRow {
            id: r.get(0)?,
            name: r.get(1)?,
            kind: r.get(2)?,
            url: r.get(3)?,
            api_key: r.get(4)?,
            enabled: r.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn insert_source(conn: &Connection, name: &str, url: &str, api_key: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO sources (name, url, api_key) VALUES (?1, ?2, ?3)",
        params![name, url, api_key],
    )?;
    Ok(())
}

pub fn set_source_enabled(conn: &Connection, id: i64, enabled: bool) -> rusqlite::Result<()> {
    conn.execute("UPDATE sources SET enabled = ?2 WHERE id = ?1", params![id, enabled])?;
    Ok(())
}

pub fn delete_source(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM sources WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_an_old_library() {
        let path = std::env::temp_dir().join(format!("cardamoi-db-upgrade-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let old = Connection::open(&path).unwrap();
            old.execute_batch(
                "CREATE TABLE items (id INTEGER PRIMARY KEY, title TEXT NOT NULL, year INTEGER,
                 source TEXT NOT NULL, info_hash TEXT NOT NULL UNIQUE, poster TEXT,
                 added_at INTEGER NOT NULL DEFAULT (unixepoch()));
                 INSERT INTO items (title, source, info_hash) VALUES ('Old', 'Internet Archive', 'aa');",
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        insert(
            &conn,
            &NewItem {
                title: "Severance",
                year: Some(2022),
                source: "Prowlarr",
                info_hash: "bb",
                poster: None,
                tmdb_id: Some(95396),
                kind: Some("tv"),
                season: Some(2),
                episode: Some(3),
                torrent_url: Some("magnet:?xt=urn:btih:bb"),
                single_video: true,
                local_path: None,
            },
        )
        .unwrap();
        let items = list(&conn).unwrap();
        assert_eq!(items.len(), 2);
        let new = items.iter().find(|i| i.info_hash == "bb").unwrap();
        assert_eq!((new.tmdb_id, new.season, new.episode), (Some(95396), Some(2), Some(3)));
        assert_eq!((new.torrent_url.as_deref(), new.single_video), (Some("magnet:?xt=urn:btih:bb"), true));
        assert_eq!(items.iter().find(|i| i.info_hash == "aa").unwrap().tmdb_id, None);
        drop(conn);
        open(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    fn temp_db(name: &str) -> (Connection, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("cardamoi-db-{name}-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        (open(&path).unwrap(), path)
    }

    fn new_item(hash: &str) -> NewItem<'_> {
        NewItem {
            title: "Severance",
            year: Some(2022),
            source: "Prowlarr",
            info_hash: hash,
            poster: None,
            tmdb_id: Some(95396),
            kind: Some("tv"),
            season: Some(1),
            episode: None,
            torrent_url: None,
            single_video: false,
            local_path: None,
        }
    }

    fn episode(file_index: i64, episode: u32) -> NewVideo {
        NewVideo {
            file_index,
            path: format!("Severance.S01E{episode:02}.mkv"),
            len: 1000,
            season: Some(1),
            episode: Some(episode),
        }
    }

    #[test]
    fn playback_is_kept_per_episode() {
        let (conn, path) = temp_db("pb");
        insert(&conn, &new_item("cc")).unwrap();
        let id = list(&conn).unwrap()[0].id;
        save_videos(&conn, id, &[episode(2, 2), episode(0, 1)], 0).unwrap();
        let videos = videos_for(&conn, id).unwrap();
        assert_eq!(videos.iter().map(|v| v.episode).collect::<Vec<_>>(), [Some(1), Some(2)]);
        assert_eq!(videos[0].position_secs, 0.0);
        assert_eq!(get_tracks(&conn, id, 0).unwrap(), None);

        save_position(&conn, id, 2, 120.5, Some(3000.0)).unwrap();
        save_position(&conn, id, 2, 130.0, None).unwrap();
        set_watched(&conn, id, 0, true).unwrap();
        let tracks = TrackPrefs { sub_lang: Some("en".into()), audio_lang: None, sub_offset_ms: -500 };
        save_tracks(&conn, id, 2, &tracks).unwrap();

        let videos = videos_for(&conn, id).unwrap();
        assert_eq!((videos[0].watched, videos[0].position_secs), (true, 0.0));
        assert_eq!((videos[1].position_secs, videos[1].duration_secs, videos[1].watched), (130.0, Some(3000.0), false));
        assert!(videos[1].last_played_at.is_some());
        assert_eq!(get_tracks(&conn, id, 2).unwrap(), Some(tracks.clone()));
        assert_eq!(get_tracks(&conn, id, 0).unwrap().unwrap().sub_lang, None);

        set_season_watched(&conn, 95396, 1, true).unwrap();
        assert!(videos_for(&conn, id).unwrap().iter().all(|v| v.watched));
        set_season_watched(&conn, 95396, 1, false).unwrap();
        assert!(videos_for(&conn, id).unwrap().iter().all(|v| !v.watched));
        assert_eq!(videos_for(&conn, id).unwrap()[1].position_secs, 130.0);

        save_show_tracks(&conn, 95396, &tracks).unwrap();
        assert_eq!(get_show_tracks(&conn, 95396).unwrap(), Some(tracks));

        set_setting(&conn, "subtitle_languages", "en,ar").unwrap();
        set_setting(&conn, "subtitle_languages", "ar").unwrap();
        assert_eq!(get_setting(&conn, "subtitle_languages").unwrap().as_deref(), Some("ar"));

        let sub = |path, file_index| NewSubtitle {
            item_id: id,
            file_index,
            lang: Some("en"),
            label: "Severance.S01E02.1080p.WEB",
            path,
            source: "opensubtitles",
            hearing_impaired: false,
            provider_key: Some("123"),
        };
        let first = insert_subtitle(&conn, &sub("/tv/Severance.S01E02.en.srt", 2)).unwrap();
        assert_eq!(insert_subtitle(&conn, &sub("/tv/Severance.S01E02.en.srt", 2)).unwrap(), first);
        insert_subtitle(&conn, &sub("/tv/Severance.S01E01.en.srt", 0)).unwrap();
        assert_eq!(list_subtitles(&conn, id, 2).unwrap(), [first]);

        delete(&conn, id).unwrap();
        assert!(videos_for(&conn, id).unwrap().is_empty());
        assert_eq!(get_tracks(&conn, id, 2).unwrap(), None);
        assert!(list_subtitles(&conn, id, 2).unwrap().is_empty());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_magnet_added_twice_is_one_download_and_gets_matched_later() {
        let (conn, path) = temp_db("magnet");
        let mut item = new_item("ee");
        item.title = "Some.Show.S01E01.1080p";
        item.tmdb_id = None;
        item.kind = None;
        item.poster = Some("old.jpg");
        let id = insert(&conn, &item).unwrap();
        assert_eq!(insert(&conn, &item).unwrap(), id);
        assert_eq!(list(&conn).unwrap().len(), 1);

        let found = Match {
            title: "Some Show",
            year: Some(2024),
            poster: None,
            tmdb_id: 42,
            kind: "tv",
            season: Some(1),
            episode: Some(1),
        };
        set_match(&conn, id, &found).unwrap();
        let got = get(&conn, id).unwrap();
        assert_eq!((got.title.as_str(), got.tmdb_id, got.episode), ("Some Show", Some(42), Some(1)));
        assert_eq!(got.poster.as_deref(), Some("old.jpg"));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn progress_kept_per_download_moves_to_its_main_video() {
        let path = std::env::temp_dir().join(format!("cardamoi-db-pbold-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let old = open(&path).unwrap();
            old.execute_batch(
                "DROP TABLE playback;
                 CREATE TABLE playback (item_id INTEGER PRIMARY KEY, position_secs REAL NOT NULL DEFAULT 0,
                     duration_secs REAL, watched INTEGER NOT NULL DEFAULT 0, sub_lang TEXT, audio_lang TEXT,
                     sub_offset_ms INTEGER NOT NULL DEFAULT 0, updated_at INTEGER NOT NULL DEFAULT (unixepoch()));
                 INSERT INTO items (id, title, source, info_hash) VALUES (7, 'Dune', 'x', 'dd');
                 INSERT INTO playback (item_id, position_secs, duration_secs, sub_lang) VALUES (7, 2467, 9000, 'ar');
                 INSERT INTO subtitles (item_id, label, path, source) VALUES (7, 'Dune', '/f/Dune.ar.srt', 'subdl');",
            )
            .unwrap();
        }
        let conn = open(&path).unwrap();
        save_videos(&conn, 7, &[NewVideo { file_index: 3, path: "Dune.mkv".into(), len: 9, season: None, episode: None }], 3)
            .unwrap();
        let video = &videos_for(&conn, 7).unwrap()[0];
        assert_eq!((video.file_index, video.position_secs, video.duration_secs), (3, 2467.0, Some(9000.0)));
        assert_eq!(get_tracks(&conn, 7, 3).unwrap().unwrap().sub_lang.as_deref(), Some("ar"));
        assert_eq!(list_subtitles(&conn, 7, 3).unwrap()[0].file_index, Some(3));
        drop(conn);
        open(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
    }

}
