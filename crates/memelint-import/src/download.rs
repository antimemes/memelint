//! The PDF of ASD-STE100, downloaded into the memelint cache when there is
//! no local copy. The URL and the hash are those of `nix/asd-ste100.nix`.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

const URL: &str = "https://www.asd-ste100.org/assets/files/ASD-STE100_ISSUE9.pdf";
const SHA256: &str = "d1f4ea9e7cd6e46b47aa9057209f99e78c0e9cfc4e27a5b07895b05c1a166431";
const FILE: &str = "ASD-STE100_ISSUE9.pdf";

/// The PDF in the cache. It is downloaded when it is not there or when it
/// is not the document that the hash gives.
pub fn asd_ste100() -> Result<PathBuf> {
    let path = memelint_common::cache_dir()
        .context("no cache directory: neither XDG_CACHE_HOME nor HOME is set")?
        .join(FILE);
    if let Ok(bytes) = std::fs::read(&path) {
        if crate::ste100::sha256_hex(&bytes) == SHA256 {
            return Ok(path);
        }
        eprintln!(
            "note: {} is not Issue 9; downloading it again",
            path.display()
        );
    }

    eprintln!("note: downloading {URL}");
    let bytes = ureq::get(URL)
        .call()
        .with_context(|| format!("cannot download {URL}"))?
        .body_mut()
        .with_config()
        .limit(64 * 1024 * 1024)
        .read_to_vec()
        .with_context(|| format!("cannot read {URL}"))?;
    let hash = crate::ste100::sha256_hex(&bytes);
    if hash != SHA256 {
        bail!("the download from {URL} is not ASD-STE100 Issue 9 (sha256 {hash})");
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }
    std::fs::write(&path, &bytes).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(path)
}
