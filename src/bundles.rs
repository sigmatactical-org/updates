//! Store and serve signed RAUC update bundles, one directory per OTA channel.
//!
//! Bundles live at `<bundles_dir>/<channel>/bundle/<name>.raucb`, mirroring
//! the download URL (`/v1/channel/<channel>/bundle/<name>`) so `warp::fs::dir`
//! can serve them directly — streamed from disk with range support, never
//! buffered in memory. Uploads stream to a temp file and are renamed into
//! place atomically.

use std::path::PathBuf;

use bytes::Buf;
use futures_util::{Stream, StreamExt};
use tokio::io::AsyncWriteExt;

use crate::config;
use crate::listing::{self, PublishError};

/// Soft cap for a single uploaded bundle (2 GiB).
pub const MAX_BUNDLE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Whether `channel` is a plain lowercase channel name (`dev`, `stable`, …).
pub fn is_safe_channel(channel: &str) -> bool {
    !channel.is_empty()
        && channel.len() <= 32
        && channel
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Whether `name` is a plain `.raucb` filename (no path traversal).
pub fn is_safe_filename(name: &str) -> bool {
    listing::is_safe_filename(name, ".raucb")
}

/// Resolve a downloadable bundle path; rejects unsafe channel/name parts.
pub fn bundle_path(channel: &str, name: &str) -> Option<PathBuf> {
    if !is_safe_channel(channel) || !is_safe_filename(name) {
        return None;
    }
    let path = config::bundles_dir()
        .join(channel)
        .join("bundle")
        .join(name);
    if path.is_file() { Some(path) } else { None }
}

/// Stream an uploaded bundle to `<bundles_dir>/<channel>/bundle/<name>`, atomically.
///
/// Returns the stored byte count. The temp file is cleaned up on any failure.
pub async fn store_bundle<S, B, E>(
    channel: &str,
    name: &str,
    mut body: S,
) -> Result<u64, PublishError>
where
    S: Stream<Item = Result<B, E>> + Unpin,
    B: Buf,
    E: std::fmt::Display,
{
    if !is_safe_channel(channel) || !is_safe_filename(name) {
        return Err(PublishError::InvalidFilename);
    }

    let dir = config::bundles_dir().join(channel).join("bundle");
    tokio::fs::create_dir_all(&dir).await?;
    let dest = dir.join(name);
    let tmp = dir.join(format!(".{name}.tmp"));

    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut written: u64 = 0;
    while let Some(chunk) = body.next().await {
        let mut chunk = match chunk {
            Ok(chunk) => chunk,
            Err(err) => {
                drop(file);
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(PublishError::InvalidContent(format!(
                    "upload aborted: {err}"
                )));
            }
        };
        written += chunk.remaining() as u64;
        if written > MAX_BUNDLE_BYTES {
            drop(file);
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(PublishError::TooLarge);
        }
        while chunk.has_remaining() {
            let part = chunk.chunk();
            file.write_all(part).await?;
            let n = part.len();
            chunk.advance(n);
        }
    }
    if written == 0 {
        drop(file);
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(PublishError::EmptyBody);
    }
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(&tmp, &dest).await?;
    Ok(written)
}

/// Remove a published bundle.
pub fn delete_bundle(channel: &str, name: &str) -> Result<(), PublishError> {
    let Some(path) = bundle_path(channel, name) else {
        return Err(PublishError::NotFound);
    };
    std::fs::remove_file(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_names() {
        assert!(is_safe_channel("dev"));
        assert!(is_safe_channel("stable-1"));
        assert!(!is_safe_channel(""));
        assert!(!is_safe_channel("Dev"));
        assert!(!is_safe_channel("../dev"));
    }

    #[test]
    fn filenames() {
        assert!(is_safe_filename("1.1.0.raucb"));
        assert!(!is_safe_filename("../1.1.0.raucb"));
        assert!(!is_safe_filename("bundle.deb"));
    }

    #[tokio::test]
    async fn stores_and_deletes_atomically() {
        let dir = tempfile::tempdir().unwrap();
        // SAFETY-free env shim: point the bundles dir at the temp dir.
        temp_env::async_with_vars(
            [("UPDATES_BUNDLES_DIR", Some(dir.path().to_str().unwrap()))],
            async {
                let chunks: Vec<Result<bytes::Bytes, std::io::Error>> = vec![
                    Ok(bytes::Bytes::from_static(b"rauc")),
                    Ok(bytes::Bytes::from_static(b"data")),
                ];
                let stream = futures_util::stream::iter(chunks);
                let n = store_bundle("dev", "1.1.0.raucb", stream).await.unwrap();
                assert_eq!(n, 8);
                let path = bundle_path("dev", "1.1.0.raucb").unwrap();
                assert_eq!(std::fs::read(&path).unwrap(), b"raucdata");
                delete_bundle("dev", "1.1.0.raucb").unwrap();
                assert!(bundle_path("dev", "1.1.0.raucb").is_none());
            },
        )
        .await;
    }
}
