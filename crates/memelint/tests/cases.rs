//! End-to-end tests: `memelint check` on the fixture worktrees.
//!
//! Each directory in `tests/cases/` is one test. Its `case.toml` names a
//! worktree in `tests/worktrees/` and the arguments for `check`.
//! `expected.txt` is the output of memelint: the findings (stdout), then the
//! summary and the notes (stderr). The exit status must be 1 when the summary
//! counts errors and 0 if it does not. The number of finding lines must
//! agree with the summary.
//!
//! Before the run, we make a copy of the worktree in a work directory. We put
//! the files of the `overlay/` directory of the case, if there is one, on
//! top. Thus some cases can lint the same worktree with different allowlists
//! or configuration. When the case has a `fixed/` directory, each file in it
//! must be the same as the file that `check --fix` left in the work
//! directory. The checker runs with the dictionary in
//! `tests/dict.json`. Its words are not English, and no part of the
//! ASD-STE100 dictionary is in these files.
//!
//! `UPDATE_EXPECT=1 cargo test --test cases` writes each `expected.txt` and
//! each `fixed/` file again from the output. Examine the diff before you commit it.

use std::fs;
use std::path::Path;
use std::process::Command;

use libtest_mimic::{Arguments, Failed, Trial};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Directory in `tests/worktrees/` to lint.
    worktree: String,
    /// More arguments for `memelint check`. Paths start at the worktree.
    #[serde(default)]
    args: Vec<String>,
}

#[cfg(not(feature = "bundled"))]
fn main() {
    eprintln!("the case tests need the bundled grammars; skipped");
}

#[cfg(feature = "bundled")]
fn main() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut dirs: Vec<_> = fs::read_dir(tests.join("cases"))
        .expect("tests/cases")
        .map(|e| e.expect("directory entry").path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    let mut trials: Vec<Trial> = dirs
        .into_iter()
        .map(|dir| {
            let name = dir.file_name().unwrap().to_string_lossy().into_owned();
            Trial::test(name, move || run(&dir))
        })
        .collect();
    trials.push(Trial::test("readme-example", move || {
        readme_example(&tests)
    }));
    libtest_mimic::run(&Arguments::from_args(), trials).exit();
}

fn run(case_dir: &Path) -> Result<(), Failed> {
    let tests = case_dir.parent().unwrap().parent().unwrap();
    let name = case_dir.file_name().unwrap().to_string_lossy();
    let case: Case = toml::from_str(&fs::read_to_string(case_dir.join("case.toml"))?)?;

    let work = std::env::temp_dir().join(format!("memelint-case-{name}"));
    let _ = fs::remove_dir_all(&work);
    copy_tree(&tests.join("worktrees").join(&case.worktree), &work)?;
    let overlay = case_dir.join("overlay");
    if overlay.is_dir() {
        copy_tree(&overlay, &work)?;
    }

    // No input from this machine: no grammar directory, and no dictionary
    // other than the fixture. Thus the output is the same on all machines.
    let output = Command::new(env!("CARGO_BIN_EXE_memelint"))
        .arg("check")
        .arg("--vocab")
        .arg(tests.join("dict.json"))
        .args(&case.args)
        .current_dir(&work)
        .env_remove("MEMELINT_VOCAB")
        .env_remove("MEMELINT_GRAMMARS")
        .env("XDG_DATA_HOME", "/nonexistent")
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    let actual = format!("{stdout}{stderr}");

    let expected_path = case_dir.join("expected.txt");
    if std::env::var_os("UPDATE_EXPECT").is_some() {
        fs::write(&expected_path, &actual)?;
    }
    let expected = fs::read_to_string(&expected_path).unwrap_or_default();
    if actual != expected {
        return Err(format!(
            "output differs from {}:\n{}\nfull output:\n{actual}",
            expected_path.display(),
            diff(&expected, &actual)
        )
        .into());
    }

    let count = |label: &str| -> Result<usize, Failed> {
        stderr
            .lines()
            .find_map(|l| l.split_once(label).map(|(before, _)| before))
            .and_then(|before| before.rsplit(' ').next()?.parse::<usize>().ok())
            .ok_or_else(|| format!("no {label} count on stderr").into())
    };
    let errors = count(" errors")?;
    let warnings = count(" warnings")?;
    let lines = stdout.lines().count();
    if lines != errors + warnings {
        return Err(format!(
            "summary counts {errors} errors and {warnings} warnings but stdout has {lines} lines"
        )
        .into());
    }
    let want = i32::from(errors > 0);
    if output.status.code() != Some(want) {
        return Err(format!("exit status {:?}, expected {want}", output.status.code()).into());
    }

    let fixed = case_dir.join("fixed");
    if fixed.is_dir() {
        for rel in files_under(&fixed, &fixed)? {
            let expected_path = fixed.join(&rel);
            let actual = fs::read_to_string(work.join(&rel))?;
            if std::env::var_os("UPDATE_EXPECT").is_some() {
                fs::write(&expected_path, &actual)?;
            }
            let expected = fs::read_to_string(&expected_path)?;
            if actual != expected {
                return Err(format!(
                    "{} differs from {}:\n{}",
                    rel.display(),
                    expected_path.display(),
                    diff(&expected, &actual)
                )
                .into());
            }
        }
    }
    Ok(())
}

/// Lines only in `expected` (-) and only in `actual` (+).
fn diff(expected: &str, actual: &str) -> String {
    let mut out = String::new();
    for l in expected
        .lines()
        .filter(|l| !actual.lines().any(|a| a == *l))
    {
        out.push_str(&format!("- {l}\n"));
    }
    for l in actual
        .lines()
        .filter(|l| !expected.lines().any(|e| e == *l))
    {
        out.push_str(&format!("+ {l}\n"));
    }
    out
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// The example output in the README and in the documentation is the output of the
/// `readme` case.
fn readme_example(tests: &Path) -> Result<(), Failed> {
    let expected = fs::read_to_string(tests.join("cases/readme/expected.txt"))?;
    let findings: Vec<&str> = expected.lines().filter(|l| l.contains("[STE ")).collect();
    for document in ["README.md", "docs/src/introduction.md"] {
        let text = fs::read_to_string(tests.join("../../..").join(document))?;
        let start = text
            .find("```console\n$ memelint check\n")
            .ok_or(format!("no `memelint check` example in {document}"))?;
        let block = &text[start + "```console\n$ memelint check\n".len()..];
        let block = &block[..block.find("```").ok_or("example block has no end")?];
        let shown: Vec<&str> = block.lines().collect();
        if shown != findings {
            return Err(format!(
                "{document} example differs from tests/cases/readme/expected.txt:\n{}",
                diff(&findings.join("\n"), &shown.join("\n"))
            )
            .into());
        }
    }
    Ok(())
}

/// The files below `dir`, as paths that start at `root`.
fn files_under(dir: &Path, root: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            out.extend(files_under(&entry.path(), root)?);
        } else {
            out.push(entry.path().strip_prefix(root).unwrap().to_path_buf());
        }
    }
    out.sort();
    Ok(out)
}
