use std::io::{Cursor, Read};
use std::path::Path;

use thiserror::Error;

use crate::control::DebControl;

#[derive(Debug, Error)]
pub enum DebError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a debian package archive: {0}")]
    Archive(String),
    #[error("control parse error: {0}")]
    Control(String),
}

/// Inspect a `.deb` file on disk.
pub fn inspect_deb_file(path: &Path) -> Result<DebControl, DebError> {
    let bytes = std::fs::read(path)?;
    inspect_deb_bytes(&bytes)
}

/// Inspect `.deb` bytes in memory.
pub fn inspect_deb_bytes(bytes: &[u8]) -> Result<DebControl, DebError> {
    let mut archive = ar::Archive::new(Cursor::new(bytes));
    let mut control_tar: Option<(String, Vec<u8>)> = None;

    while let Some(entry) = archive.next_entry() {
        let mut entry = entry.map_err(|e| DebError::Archive(e.to_string()))?;
        let name = String::from_utf8_lossy(entry.header().identifier()).into_owned();
        if name.starts_with("control.tar") {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            control_tar = Some((name, buf));
            break;
        }
    }

    let Some((name, data)) = control_tar else {
        return Err(DebError::Archive("missing control.tar.* member".into()));
    };

    let control_text = read_control_member(&name, &data)?;
    DebControl::parse(&control_text).map_err(DebError::Control)
}

fn read_control_member(archive_name: &str, data: &[u8]) -> Result<String, DebError> {
    let uncompressed: Vec<u8> = if archive_name.ends_with(".gz") {
        let mut dec = flate2::read::GzDecoder::new(data);
        let mut out = Vec::new();
        dec.read_to_end(&mut out)?;
        out
    } else if archive_name.ends_with(".xz") {
        let mut dec = xz2::read::XzDecoder::new(data);
        let mut out = Vec::new();
        dec.read_to_end(&mut out)?;
        out
    } else if archive_name.ends_with(".zst") {
        return Err(DebError::Archive(
            "control.tar.zst is not supported yet".into(),
        ));
    } else {
        // Uncompressed control.tar
        data.to_vec()
    };

    let mut tar = tar::Archive::new(Cursor::new(uncompressed));
    for entry in tar
        .entries()
        .map_err(|e| DebError::Archive(e.to_string()))?
    {
        let mut entry = entry.map_err(|e| DebError::Archive(e.to_string()))?;
        let path = entry
            .path()
            .map_err(|e| DebError::Archive(e.to_string()))?
            .to_string_lossy()
            .into_owned();
        let file_name = path.rsplit('/').next().unwrap_or(&path);
        if file_name == "control" {
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            return Ok(text);
        }
    }
    Err(DebError::Archive(
        "control file not found in control.tar".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_sample_deb() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../packages/sigma-updates-sample_0.1.0-1_all.deb");
        if !path.exists() {
            return;
        }
        let c = inspect_deb_file(&path).unwrap();
        assert_eq!(c.package, "sigma-updates-sample");
        assert_eq!(c.version, "0.1.0-1");
    }

    #[test]
    fn inspects_depdemo() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../packages/sigma-updates-depdemo_0.1.0-1_all.deb");
        if !path.exists() {
            return;
        }
        let c = inspect_deb_file(&path).unwrap();
        assert_eq!(c.package, "sigma-updates-depdemo");
        assert!(!c.depends.is_empty());
    }
}
