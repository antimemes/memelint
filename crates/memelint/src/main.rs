mod check;
mod config;
mod defaults;
mod dict;
mod directives;
mod extract;
mod fix;
mod grammars;
mod rules;

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, mpsc};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rayon::iter::{ParallelBridge, ParallelIterator};

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
        /// Write the fixes into the files: contractions in full. A file is
        /// written only when its code parses the same before and after.
        #[arg(long)]
        fix: bool,
        /// With --fix, also replace each unapproved word that has one
        /// alternative of the same part of speech. The alternatives of the
        /// spec do not have the same meaning in all contexts: examine the diff.
        #[arg(long, requires = "fix")]
        unsafe_fixes: bool,
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
            fix,
            unsafe_fixes,
        } => run_check(paths, vocab, config, grammars, fix, unsafe_fixes),
        #[cfg(feature = "fetch")]
        Command::Fetch { config } => run_fetch(config),
    }
}

fn run_check(
    paths: Vec<PathBuf>,
    vocab: Option<PathBuf>,
    config_path: PathBuf,
    grammar_dir: Option<PathBuf>,
    fix: bool,
    unsafe_fixes: bool,
) -> Result<()> {
    let vocab_path = match memelint_common::locate(vocab.as_deref(), memelint_common::ASD_STE100) {
        Ok(path) => path,
        Err(why) => make_vocabulary().context(why)?,
    };
    let dict = dict::Dictionary::load(&vocab_path)?;
    let config = config::Config::load(&config_path)?;
    let mut grammars = grammars::Grammars::new(grammar_dir.as_deref());

    let roots = if paths.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        paths
    };
    // An error in the list stops the run after the files before the error.
    let mut listing = Listing::default();
    let stop = list_files(
        &mut listing,
        &roots,
        &config,
        &config_path,
        &mut grammars,
        fix,
    )
    .err();
    let (files, groups, dict_ref, config_ref) = (&listing.files, &listing.groups, &dict, &config);

    // Each thread examines one file at a time, approximately in the sequence
    // of the list. This thread shows the results in the sequence of the list.
    install_panic_hook();
    let mut totals = Totals::default();
    let (send, receive) = mpsc::channel();
    std::thread::scope(|s| {
        s.spawn(move || {
            // When the thread that shows the results stops, the send gives an
            // error. Then stop the work.
            let _ = files.iter().enumerate().par_bridge().try_for_each_with(
                send,
                |send, (i, (path, group))| {
                    let group = &groups[*group];
                    let result =
                        catch(|| check_file(path, group, dict_ref, config_ref, fix, unsafe_fixes));
                    send.send((i, result))
                },
            );
        });
        let mut waiting = BTreeMap::new();
        let mut next = 0;
        for (i, result) in receive {
            waiting.insert(i, result);
            while let Some(result) = waiting.remove(&next) {
                totals.add(&files[next].0, result);
                next += 1;
            }
        }
    });
    if let Some(error) = stop {
        return Err(error);
    }

    let Totals {
        files,
        comments,
        errors,
        warnings,
        fixed,
        fixed_files,
        safe_left,
        unsafe_left,
        failed,
    } = totals;
    eprintln!(
        "{files} files, {comments} comments, {errors} errors, {warnings} warnings (ASD-STE100 issue {})",
        dict.issue.map_or("?".to_string(), |i| i.to_string())
    );
    if failed > 0 {
        eprintln!("error: {failed} files could not be checked");
    }
    if fix {
        eprintln!("fixed {fixed} findings in {fixed_files} files");
        if unsafe_left > 0 {
            eprintln!("note: {unsafe_left} more findings can be fixed with --unsafe-fixes");
        }
    } else if let Some(note) = fixable_note(safe_left, unsafe_left) {
        eprintln!("note: {note}");
    }
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
    if errors > 0 || failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

/// The files of one language, one set of word lists from
/// `subtree-word-lists` and one set of scopes. They have the same grammar,
/// allowlist and rules.
struct Group {
    grammar: grammars::Grammar,
    allow: check::Allowlist,
    /// The scopes, as positions in the scopes of the configuration.
    scopes: Vec<usize>,
}

/// The files to examine and their groups.
#[derive(Default)]
struct Listing {
    /// Each file in the sequence of the walk, with its group as a position
    /// in `groups`.
    files: Vec<(PathBuf, usize)>,
    groups: Vec<Group>,
}

/// Put the files below `roots` that memelint examines into `listing`, in the
/// sequence of the walk. An error stops the list. The files before the error
/// stay in `listing`.
fn list_files(
    listing: &mut Listing,
    roots: &[PathBuf],
    config: &config::Config,
    config_path: &Path,
    grammars: &mut grammars::Grammars,
    fix: bool,
) -> Result<()> {
    let config_dir = config_path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let subtree = subtree_word_lists(config, config_dir)?;
    let scopes = config.scope_matchers(config_dir)?;
    // One group for each language, each set of word lists from
    // `subtree-word-lists` and each set of scopes, made when its first file
    // comes.
    let mut group_of: HashMap<(String, Vec<usize>, Vec<usize>), usize> = HashMap::new();
    // With `--fix`, two threads must not write one file. A file comes two
    // times only when a root is in a different root. Then keep the first.
    let overlap = fix && roots_overlap(roots);
    let mut seen: HashSet<PathBuf> = HashSet::new();

    let (paths, stop) = walk_files(roots, config);
    for path in paths {
        let Some(grammar) = grammars.for_path(&path) else {
            continue;
        };
        // The scopes that name this file. The path starts at the directory
        // of the configuration file; a file that is not below it is in no
        // scope.
        let in_scopes = match relative(&path).strip_prefix(relative(config_dir)) {
            Ok(in_tree) => config.scopes_of(&scopes, in_tree),
            Err(_) => Vec::new(),
        };
        // A file given on the command line is examined when its language or
        // scope is off. The files of a directory are not.
        let named = roots.contains(&path);
        if !named && !config.checks(&grammar.name, grammar.prose, &in_scopes) {
            continue;
        }
        if overlap && !seen.insert(std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone())) {
            continue;
        }
        // The word lists of `subtree-word-lists` above this file.
        let lists: Vec<usize> = subtree
            .iter()
            .enumerate()
            .filter(|(_, (dir, _))| relative(&path).starts_with(dir))
            .map(|(i, _)| i)
            .collect();
        let key = (grammar.name.clone(), lists, in_scopes);
        let group = match group_of.get(&key) {
            Some(&i) => i,
            None => {
                let mut allow = config.allowlist(&grammar.name, config_path, &key.2)?;
                for &i in &key.1 {
                    allow.extend(&subtree[i].1);
                }
                listing.groups.push(Group {
                    grammar,
                    allow,
                    scopes: key.2.clone(),
                });
                group_of.insert(key, listing.groups.len() - 1);
                listing.groups.len() - 1
            }
        };
        listing.files.push((path, group));
    }
    stop.map_or(Ok(()), Err)
}

/// Is a root in a different root, or the same as it?
fn roots_overlap(roots: &[PathBuf]) -> bool {
    let real: Vec<PathBuf> = roots
        .iter()
        .map(|r| std::fs::canonicalize(r).unwrap_or_else(|_| r.clone()))
        .collect();
    real.iter().enumerate().any(|(i, a)| {
        real.iter()
            .enumerate()
            .any(|(j, b)| i != j && b.starts_with(a))
    })
}

/// The files below `roots` that are not ignored, in the sequence of a walk
/// that sorts the entries of each directory. An error stops the list. Then
/// the list has the files before the error.
fn walk_files(roots: &[PathBuf], config: &config::Config) -> (Vec<PathBuf>, Option<anyhow::Error>) {
    let mut files = Vec::new();
    for root in roots {
        let walk = match walker(root, config) {
            Ok(walk) => walk,
            Err(e) => return (files, Some(e)),
        };
        // The walk on threads does not follow a root that is a link to a
        // directory. The walk on one thread follows it. Thus use threads only
        // when the root is a directory and not a link.
        if std::fs::symlink_metadata(root).is_ok_and(|m| m.is_dir())
            && let Some(mut found) = walk_threads(&walk)
        {
            files.append(&mut found);
            continue;
        }
        // On one thread, the walk gives the error after the files that come
        // before it.
        for entry in walk.build() {
            match entry {
                Ok(entry) if entry.file_type().is_some_and(|t| t.is_file()) => {
                    files.push(entry.into_path());
                }
                Ok(_) => {}
                Err(e) => return (files, Some(e.into())),
            }
        }
    }
    (files, None)
}

/// The files of `walk`, sorted. Threads read the directories at the same
/// time. None when the walk has an error.
fn walk_threads(walk: &ignore::WalkBuilder) -> Option<Vec<PathBuf>> {
    let found = Mutex::new(Some(Vec::new()));
    let found_ref = &found;
    walk.build_parallel().run(|| {
        Box::new(move |entry| {
            let mut found = found_ref.lock().unwrap();
            match entry {
                Ok(entry) if entry.file_type().is_some_and(|t| t.is_file()) => {
                    if let Some(files) = found.as_mut() {
                        files.push(entry.into_path());
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    *found = None;
                    return ignore::WalkState::Quit;
                }
            }
            ignore::WalkState::Continue
        })
    });
    let mut files = found.into_inner().unwrap()?;
    files.sort();
    Some(files)
}

/// The result of the check of one file.
struct Checked {
    comments: usize,
    findings: Vec<(check::Finding, config::Severity)>,
    /// The number of fixes that were written into the file.
    fixed: usize,
    /// The fixes change the code. Thus memelint did not write them.
    refused: bool,
}

/// Examine one file. With `fix`, write its fixes into it. None when the file
/// is not UTF-8 text.
fn check_file(
    path: &Path,
    group: &Group,
    dict: &dict::Dictionary,
    config: &config::Config,
    fix: bool,
    unsafe_fixes: bool,
) -> Result<Option<Checked>> {
    let Ok(source) = std::fs::read_to_string(path) else {
        return Ok(None); // not UTF-8, thus not a source file
    };
    let examine = |source: &str| {
        lint(
            &group.grammar,
            source,
            dict,
            &group.allow,
            config,
            &group.scopes,
        )
    };
    let (mut comments, mut findings) = examine(&source);
    let (mut fixed, mut refused) = (0, false);
    if fix {
        let fixes: Vec<&check::Fix> = findings
            .iter()
            .filter_map(|(f, _)| f.fix.as_ref())
            .filter(|x| x.safe || unsafe_fixes)
            .collect();
        if !fixes.is_empty() {
            match fix::apply(&source, &fixes) {
                Some(after) if fix::same_code(&group.grammar, &source, &after) => {
                    std::fs::write(path, &after)
                        .with_context(|| format!("cannot write {}", shown(path)))?;
                    fixed = fixes.len();
                    // The positions of the other findings have moved.
                    (comments, findings) = examine(&after);
                }
                _ => refused = true,
            }
        }
    }
    Ok(Some(Checked {
        comments,
        findings,
        fixed,
        refused,
    }))
}

/// The counts for the summary.
#[derive(Default)]
struct Totals {
    files: usize,
    comments: usize,
    errors: usize,
    warnings: usize,
    fixed: usize,
    fixed_files: usize,
    /// Findings with a fix that was not applied: safe fixes, and unsafe fixes.
    safe_left: usize,
    unsafe_left: usize,
    /// Files that memelint could not examine because of an error or a panic.
    failed: usize,
}

impl Totals {
    /// Add the result of one file to the counts. Show its findings, notes
    /// and errors.
    fn add(&mut self, path: &Path, result: Result<Option<Checked>, String>) {
        let shown = shown(path);
        let checked = match result {
            Ok(Some(checked)) => checked,
            Ok(None) => return,
            Err(message) => {
                eprintln!("{shown}: error: {message}");
                self.failed += 1;
                return;
            }
        };
        self.files += 1;
        if checked.refused {
            eprintln!("note: {shown}: the fixes would change the code; not applied");
        }
        if checked.fixed > 0 {
            self.fixed += checked.fixed;
            self.fixed_files += 1;
        }
        for x in checked.findings.iter().filter_map(|(f, _)| f.fix.as_ref()) {
            if x.safe {
                self.safe_left += 1;
            } else {
                self.unsafe_left += 1;
            }
        }
        self.comments += checked.comments;
        for (f, severity) in checked.findings {
            let severity = match severity {
                config::Severity::Error => {
                    self.errors += 1;
                    "error"
                }
                _ => {
                    self.warnings += 1;
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

/// The path as memelint shows it: without `./` at its start.
fn shown(path: &Path) -> std::path::Display<'_> {
    path.strip_prefix("./").unwrap_or(path).display()
}

thread_local! {
    /// This thread examines a file in `catch`.
    static CATCHING: Cell<bool> = const { Cell::new(false) };
    /// The location and the message of the last panic in `catch`.
    static PANIC: Cell<Option<String>> = const { Cell::new(None) };
}

/// Keep the message of a panic in `catch` for its error, and do not write
/// it. Send all other panics to the function that the process had before.
fn install_panic_hook() {
    let before = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if CATCHING.get() {
            // The text has the location and then the message, on one line.
            PANIC.set(Some(info.to_string().replace('\n', " ")));
        } else {
            before(info);
        }
    }));
}

/// Do the work for one file. An error or a panic becomes a message. Thus
/// one file cannot stop the check of the other files.
fn catch<T>(work: impl FnOnce() -> Result<T>) -> Result<T, String> {
    CATCHING.set(true);
    let result = panic::catch_unwind(AssertUnwindSafe(work));
    CATCHING.set(false);
    match result {
        Ok(result) => result.map_err(|e| format!("{e:#}")),
        Err(_) => Err(PANIC.take().unwrap_or_else(|| "panicked".to_string())),
    }
}

/// The walk of the files below `root` that are not ignored. On one thread,
/// the sequence is the same on each filesystem.
fn walker(root: &Path, config: &config::Config) -> Result<ignore::WalkBuilder> {
    let mut walk = ignore::WalkBuilder::new(root);
    // A glob with `!` at the start is ignored. A glob without it is accepted.
    let mut overrides = ignore::overrides::OverrideBuilder::new(".");
    for glob in &config.ignore {
        overrides.add(&format!("!{glob}"))?;
    }
    walk.overrides(overrides.build()?);
    walk.sort_by_file_path(Ord::cmp);
    Ok(walk)
}

/// A path without the `./` at its start, to compare it with other paths.
fn relative(path: &Path) -> &Path {
    path.strip_prefix(".").unwrap_or(path)
}

/// The word lists that `subtree-word-lists` names, with the directory of
/// each. All directories below the configuration file are examined, because
/// a list can be above the paths given on the command line.
fn subtree_word_lists(
    config: &config::Config,
    config_dir: &Path,
) -> Result<Vec<(PathBuf, String)>> {
    if config.subtree_word_lists.is_empty() {
        return Ok(Vec::new());
    }
    let globs = config.subtree_globs()?;
    let mut lists = Vec::new();
    let (files, stop) = walk_files(&[config_dir.to_path_buf()], config);
    for file in files {
        let path = relative(&file);
        let Ok(in_tree) = path.strip_prefix(relative(config_dir)) else {
            continue;
        };
        if !globs.is_match(in_tree) {
            continue;
        }
        let text = std::fs::read_to_string(&file)
            .with_context(|| format!("cannot read {}", file.display()))?;
        let dir = path.parent().unwrap_or(Path::new("")).to_path_buf();
        lists.push((dir, text));
    }
    stop.map_or(Ok(lists), Err)
}

/// How many findings `--fix` can remove, and `--fix --unsafe-fixes`.
fn fixable_note(safe: usize, unsafe_: usize) -> Option<String> {
    match (safe, unsafe_) {
        (0, 0) => None,
        (s, 0) => Some(format!("{s} findings can be fixed with --fix")),
        (0, u) => Some(format!(
            "{u} findings can be fixed with --fix --unsafe-fixes"
        )),
        (s, u) => Some(format!(
            "{s} findings can be fixed with --fix, and {u} more with --fix --unsafe-fixes"
        )),
    }
}

/// The number of comments in `source`, and its findings with their
/// severities in the scopes `scopes` (positions in the scopes of `config`).
/// Findings of rules that are off are not in the list.
fn lint(
    grammar: &grammars::Grammar,
    source: &str,
    dict: &dict::Dictionary,
    allow: &check::Allowlist,
    config: &config::Config,
    scopes: &[usize],
) -> (usize, Vec<(check::Finding, config::Severity)>) {
    let comments = extract::extract(grammar, source);
    let directives = directives::parse(source);
    let findings = comments
        .iter()
        .flat_map(|c| {
            check::check(c, dict, allow)
                .into_iter()
                .filter(|f| !directives::hides(&directives, c, &comments, f.rule))
        })
        .filter_map(|f| match config.severity(f.rule, &grammar.name, scopes) {
            config::Severity::Off => None,
            severity => Some((f, severity)),
        })
        .collect();
    (comments.len(), findings)
}

#[cfg(feature = "fetch")]
fn run_fetch(config_path: PathBuf) -> Result<()> {
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

/// Makes the ASD-STE100 vocabulary with `memelint-import`, which is adjacent to
/// this binary or on PATH. The importer downloads the PDF and writes the
/// vocabulary into the cache. Thus the first run on a machine makes it.
fn make_vocabulary() -> Result<PathBuf> {
    let importer = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("memelint-import")))
        .map(|p| p.with_extension(std::env::consts::EXE_EXTENSION))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("memelint-import"));
    eprintln!("note: no ASD-STE100 vocabulary in the cache; memelint-import makes it");
    let status = std::process::Command::new(&importer)
        .args([memelint_common::ASD_STE100, "--quiet"])
        .status()
        .with_context(|| format!("cannot run {}", importer.display()))?;
    if !status.success() {
        anyhow::bail!("{} failed ({status})", importer.display());
    }
    memelint_common::locate(None, memelint_common::ASD_STE100).map_err(anyhow::Error::msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_or_an_error_becomes_a_message() {
        install_panic_hook();
        let message = catch(|| -> Result<()> { panic!("the gantry fell") }).unwrap_err();
        assert!(message.starts_with("panicked at "), "{message}");
        assert!(message.ends_with(": the gantry fell"), "{message}");
        let error = catch(|| -> Result<()> { Err(anyhow::anyhow!("no gantry")).context("lift") });
        assert_eq!(error, Err("lift: no gantry".to_string()));
        assert_eq!(catch(|| Ok(7)), Ok(7));
        // After `catch`, the thread does not examine a file.
        assert!(!CATCHING.get());
    }
}
