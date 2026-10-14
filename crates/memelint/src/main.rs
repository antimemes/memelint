mod check;
mod config;
mod defaults;
mod dict;
mod extract;
mod grammars;
mod rules;

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Simplified Technical English (ASD-STE100) lint for code comments and prose.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check the comments in source files against the STE dictionary.
    Check {
        /// Files or directories (default: `.`).
        paths: Vec<PathBuf>,
        /// Vocabulary file written by `memelint-import` (default: $MEMELINT_VOCAB,
        /// then the memelint cache directory).
        #[arg(long)]
        vocab: Option<PathBuf>,
        /// Project configuration: accepted words and paths to ignore.
        #[arg(long, default_value = config::CONFIG_FILE)]
        config: PathBuf,
        /// Directory of tree-sitter grammars (`<name>.so`). It is examined
        /// before $MEMELINT_GRAMMARS and $XDG_DATA_HOME/memelint/grammars.
        #[arg(long)]
        grammars: Option<PathBuf>,
    },
    /// Download the word lists that `memelint.toml` gives as URLs into the cache.
    #[cfg(feature = "fetch")]
    Fetch {
        /// Project configuration.
        #[arg(long, default_value = config::CONFIG_FILE)]
        config: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check {
            paths,
            vocab,
            config,
            grammars,
        } => run_check(paths, vocab, config, grammars),
        #[cfg(feature = "fetch")]
        Command::Fetch { config } => run_fetch(config),
    }
}

fn run_check(
    paths: Vec<PathBuf>,
    vocab: Option<PathBuf>,
    config_path: PathBuf,
    grammar_dir: Option<PathBuf>,
) -> Result<()> {
    let vocab_path = memelint_common::locate(vocab.as_deref(), memelint_common::ASD_STE100)
        .map_err(anyhow::Error::msg)?;
    let dict = dict::Dictionary::load(&vocab_path)?;
    let config = config::Config::load(&config_path)?;
    // One allowlist for each language, made when its first file comes.
    let mut allowlists: HashMap<String, check::Allowlist> = HashMap::new();
    let mut grammars = grammars::Grammars::new(grammar_dir.as_deref());

    let roots = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths
    };
    let mut walk = ignore::WalkBuilder::new(&roots[0]);
    // A glob with `!` at the start is ignored. A glob without it is accepted.
    let mut overrides = ignore::overrides::OverrideBuilder::new(".");
    for glob in &config.ignore {
        overrides.add(&format!("!{glob}"))?;
    }
    walk.overrides(overrides.build()?);
    walk.sort_by_file_path(Ord::cmp); // the same sequence on each filesystem
    for root in &roots[1..] {
        walk.add(root);
    }

    let (mut files, mut comments, mut errors, mut warnings) = (0usize, 0usize, 0usize, 0usize);
    for entry in walk.build() {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Some(grammar) = grammars.for_path(path) else {
            continue;
        };
        let Ok(source) = std::fs::read_to_string(path) else {
            continue; // not UTF-8, thus not a source file
        };
        files += 1;
        if !allowlists.contains_key(&grammar.name) {
            let allow = config.allowlist(&grammar.name, &config_path)?;
            allowlists.insert(grammar.name.clone(), allow);
        }
        let allow = &allowlists[&grammar.name];
        let shown = path.strip_prefix("./").unwrap_or(path).display();
        for comment in extract::extract(&grammar, &source) {
            comments += 1;
            for f in check::check(&comment, &dict, allow) {
                let severity = match config.severity(f.rule, &grammar.name) {
                    config::Severity::Off => continue,
                    config::Severity::Error => {
                        errors += 1;
                        "error"
                    }
                    config::Severity::Warning => {
                        warnings += 1;
                        "warning"
                    }
                };
                println!(
                    "{shown}:{}:{}: {severity}: {} [STE {}]",
                    f.line, f.col, f.message, f.rule
                );
            }
        }
    }

    eprintln!(
        "{files} files, {comments} comments, {errors} errors, {warnings} warnings (ASD-STE100 issue {})",
        dict.issue.map_or("?".to_string(), |i| i.to_string())
    );
    if !grammars.missing.is_empty() {
        let names: Vec<&str> = grammars.missing.iter().map(String::as_str).collect();
        let dirs: Vec<String> = grammars
            .dirs()
            .iter()
            .map(|d| d.display().to_string())
            .collect();
        eprintln!(
            "note: no grammar for {}; looked for <name>.so in {}",
            names.join(", "),
            dirs.join(", ")
        );
    }
    if errors > 0 {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(feature = "fetch")]
fn run_fetch(config_path: PathBuf) -> Result<()> {
    use anyhow::Context;
    let config = config::Config::load(&config_path)?;
    let urls = config.urls();
    if urls.is_empty() {
        eprintln!("no URLs in {}", config_path.display());
        return Ok(());
    }
    for url in urls {
        let path = config::cached_path(url).context("no cache directory: HOME is not set")?;
        let text = ureq::get(url)
            .call()
            .with_context(|| format!("cannot fetch {url}"))?
            .body_mut()
            .read_to_string()
            .with_context(|| format!("cannot read {url}"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, &text).with_context(|| format!("cannot write {}", path.display()))?;
        eprintln!("{url} -> {}", path.display());
    }
    Ok(())
}
