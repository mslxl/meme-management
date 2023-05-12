use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension};
use serde::{de::Visitor, Deserialize, Serialize};
use sha256::try_digest;

use crate::{
    error::Error,
    handler::database::{Meme, Tag, DATABASE_FILE_DIR},
    search::build_search_sql,
};

static DATABASE_VERSION: u32 = 1;

/// Create database if not exists
/// Update database if table version < [DATABASE_VERSION]
pub fn handle_version(conn: &mut Connection) -> Result<(), Error> {
    let transaction = conn.transaction().unwrap();
    transaction.execute(include_str!("create_tableversion.sql"), ())?;

    let version: Option<u32> = transaction
        .query_row(
            "SELECT version_code FROM table_version WHERE id = 1;",
            (),
            |row| Ok(row.get(0)?),
        )
        .optional()?;
    if let Some(version) = version {
        // old database
        if version < DATABASE_VERSION {
            // upgrade version 0 -> 1
            if version < 1 {
                println!("Upgrade database to version 1");
                transaction.execute_batch(include_str!("upgrade_0_1.sql"))?;
            }
        }
    } else {
        // new database
        transaction.execute(
            "INSERT INTO table_version(id, version_code) VALUES (?1, ?2);",
            (1, DATABASE_VERSION),
        )?;
        transaction.execute_batch(include_str!("create_database.sql"))?;
    }
    transaction.commit().map_err(|x| x.into())
}

/// Get database code in file
pub fn query_table_version_code(conn: &Connection) -> Result<i64, Error> {
    let res = conn.query_row(
        "SELECT version_code FROM table_version WHERE id = 1;",
        (),
        |row| Ok(row.get(0).unwrap()),
    )?;

    Ok(res)
}

/// Insert tag into database, or donothing if it already exists
pub fn query_or_insert_tag(conn: &Connection, namespace: &str, value: &str) -> Result<i64, Error> {
    let tag_id = query_tag_id(conn, namespace, value)?;

    if let Some(id) = tag_id {
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO tag(namespace, value) VALUES (?1, ?2);",
            (namespace, value),
        )?;
        Ok(conn.last_insert_rowid())
    }
}

/// Add tag to meme
/// Tag info is store in other table
pub fn link_tag_meme(conn: &Connection, tag_id: i64, meme_id: i64) -> Result<(), Error> {
    conn.execute(
        "INSERT OR IGNORE INTO meme_tag(tag_id, meme_id) VALUES (?1, ?2) ",
        (tag_id, meme_id),
    )?;
    Ok(())
}

pub fn unlink_tag_meme(
    conn: &Connection,
    tag_id: i64,
    meme_id: i64,
    remove_unused_tag: bool,
) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM meme_tag WHERE tag_id = ?1 AND meme_id = ?2",
        [tag_id, meme_id],
    )?;
    if remove_unused_tag {
        let num: usize = conn.query_row(
            "SELECT COUNT(*) FROM meme_tag WHERE tag_id = ?1",
            [tag_id],
            |row| Ok(row.get(0).unwrap()),
        )?;
        if num == 0 {
            conn.execute("DELETE FROM tag WHERE id = ?1", [tag_id])?;
        }
    }
    Ok(())
}

/// Add meme basic info to database
pub fn insert_meme(
    conn: &Connection,
    file_id: String,
    extra_data: Option<String>,
    summary: String,
    description: Option<String>,
    thumbnail: Option<String>,
) -> Result<i64, Error> {
    conn.execute(
            "INSERT INTO meme(content, extra_data, summary, desc, thumbnail) VALUES (?1, ?2, ?3, ?4, ?5)",
            (file_id, extra_data, summary, description, thumbnail))?;
    Ok(conn.last_insert_rowid())
}

pub fn update_meme_edit_time(conn: &Connection, id: i64) -> Result<(), Error> {
    conn.execute(
        "UPDATE meme SET update_time = CURRENT_TIMESTAMP WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn update_meme(
    conn: &Connection,
    id: i64,
    extra_data: Option<String>,
    summary: Option<String>,
    description: Option<String>,
    thumbnail: Option<String>,
) -> Result<(), Error> {
    if let Some(extra_data) = extra_data {
        conn.execute(
            "UPDATE meme SET extra_data = ?2 WHERE id = ?1",
            (id, extra_data),
        )?;
    }
    if let Some(summary) = summary {
        conn.execute("UPDATE meme SET summary = ?2 WHERE id = ?1", (id, summary))?;
    }
    if let Some(description) = description {
        conn.execute("UPDATE meme SET desc = ?2 WHERE id = ?1", (id, description))?;
    }
    if let Some(thumbnail) = thumbnail {
        conn.execute(
            "UPDATE meme SET thumbnail = ?2 WHERE id = ?1",
            (id, thumbnail),
        )?;
    }
    Ok(())
}

pub fn query_meme_by_id(conn: &Connection, id: i64) -> Result<Meme, Error> {
    let result = conn.query_row(
        "SELECT id, content, extra_data, summary, desc, fav, trash FROM meme WHERE id = ?1",
        [id],
        |row| {
            Ok(Meme {
                id: row.get(0).unwrap(),
                content: row.get(1).unwrap(),
                extra_data: row.get(2).ok(),
                summary: row.get(3).unwrap(),
                desc: row.get(4).unwrap(),
                fav: row.get(5).unwrap(),
                trash: row.get(6).unwrap(),
            })
        },
    )?;
    Ok(result)
}

pub fn update_fav_meme_by_id(conn: &Connection, id: i64, value: bool) -> Result<(), Error>{
    conn.execute("UPDATE meme SET fav = ?2 WHERE id = ?1", (id, value))?;
    Ok(())
}

pub enum SearchMode {
    OnlyFav,
    OnlyTrash,
    Normal,
}

impl SearchMode {
    fn where_stmt(&self) -> &'static str {
        match self {
            SearchMode::OnlyTrash => "trash = 1",
            SearchMode::OnlyFav => "fav = 1 AND trash != 1",
            SearchMode::Normal => "trash != 1",
        }
    }
}

impl Serialize for SearchMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            SearchMode::OnlyFav => serializer.serialize_str("OnlyFav"),
            SearchMode::OnlyTrash => serializer.serialize_str("OnlyTrash"),
            SearchMode::Normal => serializer.serialize_str("Normal"),
        }
    }
}

struct SearchModeVisitor;
impl<'de> Visitor<'de> for SearchModeVisitor {
    type Value = SearchMode;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("must be a string OnlyFav, OnlyTrash or Normal")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match v {
            "OnlyFav" => Ok(SearchMode::OnlyFav),
            "OnlyTrash" => Ok(SearchMode::OnlyTrash),
            "Normal" => Ok(SearchMode::Normal),
            _ => Err(E::custom("must be a string OnlyFav, OnlyTrash or Normal"))
        }
    }
}

impl<'de> Deserialize<'de> for SearchMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_str(SearchModeVisitor)
    }
}

pub fn search_meme_by_stmt(
    conn: &Connection,
    stmt: &str,
    page: i32,
    mode: SearchMode,
) -> Result<Vec<Meme>, Error> {
    let mut stmt = build_search_sql(stmt)?;
    stmt.push_str(&format!(
        "{} ORDER BY update_time DESC LIMIT 30 OFFSET {};",
        mode.where_stmt(),
        page * 30
    ));
    let mut stmt = conn.prepare(&stmt).unwrap();
    let iter = stmt.query_map([], |row| {
        Ok(Meme {
            id: row.get("id").unwrap(),
            content: row.get("content").unwrap(),
            extra_data: row.get("extra_data").ok(),
            summary: row.get("summary").unwrap(),
            desc: row.get("desc").unwrap(),
            fav: row.get("fav").unwrap(),
            trash: row.get("trash").unwrap(),
        })
    })?;
    let mut memes = Vec::new();
    for m in iter {
        memes.push(m?);
    }
    Ok(memes)
}

pub fn query_all_meme_tag(conn: &Connection, id: i64) -> Result<Vec<Tag>, Error> {
    let mut stmt = conn.prepare("SELECT tag.namespace, tag.value FROM meme_tag LEFT JOIN tag on meme_tag.tag_id = tag.id WHERE meme_tag.meme_id = ?1").unwrap();
    let iter = stmt
        .query_map([id], |row| {
            Ok(Tag {
                namespace: row.get(0).unwrap(),
                value: row.get(1).unwrap(),
            })
        })
        .unwrap();

    let mut tags = Vec::new();
    for tag in iter {
        tags.push(tag?);
    }
    Ok(tags)
}

pub fn query_tag_namespace_with_prefix(
    conn: &Connection,
    prefix: &str,
) -> Result<Vec<String>, Error> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT namespace FROM tag WHERE namespace LIKE ?1")
        .unwrap();
    let iter = stmt
        .query_map([format!("{}%", prefix)], |row| Ok(row.get(0).unwrap()))
        .unwrap();

    let mut namespace = Vec::new();

    for nsp in iter {
        namespace.push(nsp?);
    }
    Ok(namespace)
}
pub fn query_tag_value_fuzzy(conn: &Connection, kwd: &str) -> Result<Vec<Tag>, Error> {
    let mut stmt = conn
        .prepare("SELECT namespace, value FROM tag WHERE value LIKE ?1")
        .unwrap();
    let iter = stmt
        .query_map([format!("{}%", kwd)], |row| {
            Ok(Tag {
                namespace: row.get("namespace").unwrap(),
                value: row.get("value").unwrap(),
            })
        })
        .unwrap();

    let mut tags = Vec::new();
    for tag in iter {
        tags.push(tag?);
    }
    Ok(tags)
}

pub fn query_tag_value_with_prefix(
    conn: &Connection,
    namespace: &str,
    prefix: &str,
) -> Result<Vec<String>, Error> {
    let mut stmt = conn
        .prepare("SELECT DISTINCT value FROM tag WHERE namespace = ?1 AND value LIKE ?2")
        .unwrap();
    let iter = stmt.query_map([namespace, &format!("{}%", prefix)], |row| {
        Ok(row.get(0).unwrap())
    })?;

    let mut tag_value = Vec::new();
    for v in iter {
        tag_value.push(v?);
    }
    Ok(tag_value)
}

pub fn query_count_memes(conn: &Connection) -> Result<i64, Error> {
    conn.query_row("SELECT COUNT(id) FROM meme", [], |v| Ok(v.get(0).unwrap()))
        .map_err(Error::from)
}

pub fn query_count_tags(conn: &Connection) -> Result<i64, Error> {
    conn.query_row("SELECT COUNT(id) FROM tag", [], |v| Ok(v.get(0).unwrap()))
        .map_err(Error::from)
}

pub fn query_tag_id(conn: &Connection, namespace: &str, value: &str) -> Result<Option<i64>, Error> {
    let id: Option<i64> = conn
        .query_row(
            "SELECT id FROM tag WHERE namespace = ?1 AND value = ?2",
            (namespace, value),
            |row| Ok(row.get(0).unwrap()),
        )
        .optional()?;
    Ok(id)
}

fn add_file_to_library<P: AsRef<Path>>(file: P) -> Result<String, Error> {
    let sha256 = try_digest(file.as_ref())?;
    let target = DATABASE_FILE_DIR.join(&sha256);
    fs::copy(file, target)?;
    Ok(sha256)
}

pub fn add_file(file: String, delete_after_add: bool) -> Result<String, Error> {
    let path = PathBuf::from(file);
    let sha256 = add_file_to_library(&path)?;
    if delete_after_add {
        fs::remove_file(path)?;
    }
    Ok(sha256)
}
