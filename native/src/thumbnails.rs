use std::collections::HashSet;
use std::path::PathBuf;

use crate::models::media::AppMediaSession;

const ART_URL_PREFIX: &str = "streamq-local://thumbs/";
const ART_DIR_NAME: &str = "streamq";
const ART_FILE_PREFIX: &str = "thumb_";

pub struct Thumbnails;

impl Thumbnails {
    // URL: `streamq-local://thumbs/{id}{ext}`
    // File: `{tmpdir}/streamq/thumb_{id}{ext}`
    pub fn path_for(app: &str, title: Option<&str>, artist: Option<&str>, ext: &str) -> Option<(String, PathBuf)> {
        let name = format!("{}{ext}", track_id(app, title, artist));
        let url = format!("{ART_URL_PREFIX}{name}");
        let path = art_dir()?.join(format!("{ART_FILE_PREFIX}{name}"));
        Some((url, path))
    }

    pub fn release_unreferenced(previous: &[AppMediaSession], live: &[AppMediaSession]) {
        let live_arts: HashSet<&str> = live.iter().filter_map(|s| s.art.as_deref()).collect();
        for prev in previous {
            if let Some(art) = prev.art.as_deref() {
                if is_managed(art) && !live_arts.contains(art) {
                    delete_art(art);
                }
            }
        }
    }
}

fn track_id(app: &str, title: Option<&str>, artist: Option<&str>) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(app.to_lowercase().as_bytes());
    hasher.update(&[0]);
    hasher.update(title.unwrap_or("").as_bytes());
    hasher.update(&[0]);
    hasher.update(artist.unwrap_or("").as_bytes());
    hasher.finalize().to_hex()[..16].to_string()
}

fn is_managed(url: &str) -> bool {
    url.starts_with(ART_URL_PREFIX)
}

fn art_dir() -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(ART_DIR_NAME);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn path_from_url(url: &str) -> Option<PathBuf> {
    let name = url.strip_prefix(ART_URL_PREFIX)?.trim_end_matches('/');
    if name.is_empty() || name.contains(['/', '\\']) || name.contains("..") {
        return None;
    }
    Some(art_dir()?.join(format!("{ART_FILE_PREFIX}{name}")))
}

fn delete_art(url: &str) {
    if let Some(path) = path_from_url(url) {
        let _ = std::fs::remove_file(path);
    }
}
