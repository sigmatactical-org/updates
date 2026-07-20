//! Extract control metadata from `.deb` archives.

mod deb_error;

pub use deb_error::DebError;

use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::Path;

use crate::control::DebControl;

/// Inspect a `.deb` file on disk, streaming the `ar` archive off the file
/// rather than loading the whole package into memory.
pub fn inspect_deb_file(path: &Path) -> Result<DebControl, DebError> {
    inspect_deb_read(BufReader::new(File::open(path)?))
}

/// Inspect `.deb` bytes in memory.
pub fn inspect_deb_bytes(bytes: &[u8]) -> Result<DebControl, DebError> {
    inspect_deb_read(Cursor::new(bytes))
}

/// Inspect a `.deb` archive from any reader. The `ar` and `tar` members are
/// streamed; only the control file text is buffered.
pub fn inspect_deb_read<R: Read>(reader: R) -> Result<DebControl, DebError> {
    let mut archive = ar::Archive::new(reader);
    while let Some(entry) = archive.next_entry() {
        let entry = entry.map_err(|e| DebError::Archive(e.to_string()))?;
        let name = String::from_utf8_lossy(entry.header().identifier()).into_owned();
        if name.starts_with("control.tar") {
            let control_text = read_control_member(&name, entry)?;
            return DebControl::parse(&control_text).map_err(DebError::Control);
        }
    }
    Err(DebError::Archive("missing control.tar.* member".into()))
}

fn read_control_member(archive_name: &str, data: impl Read) -> Result<String, DebError> {
    let uncompressed: Box<dyn Read> = if archive_name.ends_with(".gz") {
        Box::new(flate2::read::GzDecoder::new(data))
    } else if archive_name.ends_with(".xz") {
        Box::new(xz2::read::XzDecoder::new(data))
    } else if archive_name.ends_with(".zst") {
        return Err(DebError::Archive(
            "control.tar.zst is not supported yet".into(),
        ));
    } else {
        // Uncompressed control.tar
        Box::new(data)
    };

    let mut tar = tar::Archive::new(uncompressed);
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
