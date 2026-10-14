//! Import the ASD-STE100 Part 2 dictionary from the official PDF.
//!
//! The dictionary is copyrighted by ASD: free to download, not free to
//! redistribute. This module turns a locally obtained copy of the PDF into a
//! vocabulary file for local use. Neither the PDF nor the file is ever
//! bundled into a memelint binary or committed to the repository.

pub mod parse;
pub mod pdf;

use std::path::Path;

use anyhow::{Context, Result, bail};
use memelint_common::{Counts, Source, Vocabulary};

pub const NOTICE: &str = "Derived from ASD-STE100, copyright ASD, Brussels. Generated for local use only. \
                          Do not redistribute this file.";

/// Parse the PDF at `path` into a vocabulary. Parse warnings go to `warn`.
pub fn import(path: &Path, warn: &mut dyn FnMut(String)) -> Result<Vocabulary> {
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    let pages = pdf::extract_pages(&bytes)?;
    let parsed = parse::parse(&pages, warn);
    if parsed.dictionary_pages == 0 {
        bail!(
            "{} does not look like ASD-STE100: no 'Part 2 - Dictionary' pages found",
            path.display()
        );
    }
    let approved = parsed
        .entries
        .iter()
        .filter(|e| e.status == "approved")
        .count();
    Ok(Vocabulary {
        standard: "ASD-STE100".to_string(),
        issue: parsed.issue,
        issue_date: parsed.issue_date,
        source: Some(Source {
            pdf: path.display().to_string(),
            pdf_sha256: sha256_hex(&bytes),
        }),
        notice: Some(NOTICE.to_string()),
        counts: Some(Counts {
            entries: parsed.entries.len(),
            approved,
            unapproved: parsed.entries.len() - approved,
            dictionary_pages: parsed.dictionary_pages,
        }),
        entries: parsed.entries,
    })
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}
