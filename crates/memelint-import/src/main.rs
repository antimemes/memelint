mod ste100;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

/// Build a memelint vocabulary file from the document of a standard.
///
/// The documents are free to download but they have copyright. Thus the
/// file that this tool writes stays on this machine. It goes to the memelint
/// cache directory, and you must not commit it or send it to other persons.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// ASD-STE100 (Simplified Technical English), from the PDF that ASD supplies.
    AsdSte100 {
        /// Path to the ASD-STE100 PDF (default: $STE100_PDF).
        #[arg(long, env = "STE100_PDF")]
        pdf: PathBuf,
        /// Where to write the vocabulary (default: the memelint cache).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Do not show the parse warnings.
        #[arg(short, long)]
        quiet: bool,
    },
    /// Debug aid: print one PDF page as lines of words with positions.
    DumpPdf {
        /// Path to the PDF (default: $STE100_PDF).
        #[arg(long, env = "STE100_PDF")]
        pdf: PathBuf,
        /// 1-based page number.
        page: u32,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::AsdSte100 { pdf, out, quiet } => {
            let out = match out {
                Some(p) => p,
                None => memelint_common::default_path(memelint_common::ASD_STE100)
                    .context("no cache directory: neither XDG_CACHE_HOME nor HOME is set")?,
            };
            let mut warnings = Vec::new();
            let vocab = ste100::import(&pdf, &mut |w| warnings.push(w))?;

            if let Some(dir) = out.parent() {
                std::fs::create_dir_all(dir)
                    .with_context(|| format!("cannot create {}", dir.display()))?;
            }
            let json = serde_json::to_string_pretty(&vocab)?;
            std::fs::write(&out, json + "\n")
                .with_context(|| format!("cannot write {}", out.display()))?;

            if !quiet {
                for w in &warnings {
                    eprintln!("warning: {w}");
                }
            }
            let c = vocab.counts.as_ref().expect("importer fills counts");
            eprintln!(
                "{} issue {} ({}): {} entries, {} approved, {} unapproved, {} pages, {} warnings -> {}",
                vocab.standard,
                vocab.issue.map_or("?".to_string(), |i| i.to_string()),
                vocab.issue_date.as_deref().unwrap_or("?"),
                c.entries,
                c.approved,
                c.unapproved,
                c.dictionary_pages,
                warnings.len(),
                out.display()
            );
            Ok(())
        }
        Command::DumpPdf { pdf, page } => {
            let bytes =
                std::fs::read(&pdf).with_context(|| format!("cannot read {}", pdf.display()))?;
            let pages = ste100::pdf::extract_pages(&bytes)?;
            let Some(p) = pages.iter().find(|p| p.number == page) else {
                anyhow::bail!("no page {page} (document has {} pages)", pages.len());
            };
            for line in &p.lines {
                let words: Vec<String> = line
                    .words
                    .iter()
                    .map(|w| format!("{}@{:.1}", w.text, w.x0))
                    .collect();
                println!("y={:7.2}  {}", line.y, words.join("  "));
            }
            Ok(())
        }
    }
}
