//! Downloading scraped art to disk — shared by the `library` CLI batch scrape
//! and the shelf's on-demand scrape.

use std::path::Path;

use crate::catalog::ArtPaths;
use crate::screenscraper::{Client, GameInfo};

/// Fetch cover / texture / wheel for `info` into `art_dir` as `<id>-<kind>.png`
/// (`id` is normally the ROM's SHA1). A media that is absent, or that fails to
/// download, is simply left `None`.
pub fn download_art(client: &Client, art_dir: &Path, id: &str, info: &GameInfo) -> ArtPaths {
    let mut out = ArtPaths::default();
    for (url, kind, slot) in [
        (&info.cover_url, "cover", &mut out.cover),
        (&info.texture_url, "texture", &mut out.texture),
        (&info.wheel_url, "wheel", &mut out.wheel),
    ] {
        let Some(url) = url else { continue };
        let dest = art_dir.join(format!("{id}-{kind}.png"));
        match client.download(url, &dest) {
            Ok(()) => *slot = Some(dest.to_string_lossy().into_owned()),
            Err(e) => log::warn!("art {kind}: {e}"),
        }
    }
    out
}
