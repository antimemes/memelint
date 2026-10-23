# memelint

[![documentation](https://img.shields.io/badge/docs-blue)](https://memelint.antimemetics.institute)

memelint lints the memes of a project: the words that spread through comments, docstrings, and documentation. It tells you which words and sentences do not follow Simplified Technical English (ASD-STE100), with an allowlist for the vocabulary of your project.

```console
$ memelint check
foo.py:2:29: error: 'ensure' is not approved; use 'make sure' [STE 1.1]
foo.py:3:7: error: 'don't' is a contraction; write the words in full [STE 4.2]
foo.py:3:13: error: 'delve' is not in the STE dictionary; if it is a technical name, add it to memelint.toml [STE 1.6]
foo.py:4:7: error: sentence has 28 words; the STE limit for descriptive text is 25 [STE 6.3]
foo.py:4:7: error: 'closing' is the -ing form of 'close'; STE allows only the infinitive, imperative, past tense and past participle [STE 3.5]
```

`memelint check --fix` writes safe fixes automatically: each contraction becomes the words in full. `--unsafe-fixes` also replaces each unapproved word with the one alternative of the same part of speech; examine the diff.

`memelint check --format json` writes the findings as JSON on stdout. The summary stays on stderr. `--format sarif` writes SARIF 2.1.0. The default `--format text` is the line form above.

## Usage

The release binaries are for Linux and macOS, on x86_64 and aarch64:

```bash
curl -fsSL https://memelint.sh | sh
memelint check
```

With [Nix](https://nixos.org/download/), there is no installation. This builds the checker with the ASD-STE100 vocabulary and with each tree-sitter grammar that nixpkgs has, and runs it:

```bash
$(nix-build --no-out-link \
  https://github.com/antimemes/memelint/archive/main.tar.gz)\
/bin/memelint check
```

With flakes:

```bash
nix run github:antimemes/memelint -- check
```

In GitHub Actions, put this in `.github/workflows/memelint.yml`. The findings become annotations on the pull request:

```yaml
name: memelint

on:
  push:
    branches: [main]
  pull_request:

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: antimemes/memelint@latest
```

## Rules

ASD-STE100 Issue 9 has 53 rules. A program can check a part of them. memelint checks these, and each finding gives the rule id:

| Rule | Finding |
|---|---|
| 1.1 | a word that is not approved, with the approved alternatives, for example `commence` (use `start`) |
| 1.2 | an approved word in a part of speech in which it is not approved. For example, `test` is a noun, thus "do a test" and not "test it" |
| 1.6 | a word that is not in the dictionary and not in your word list, for example `kubelet`. Put the words of your project in `memelint.toml`, and they are approved |
| 3.5 | the `-ing` form of an approved verb, for example `starting` |
| 4.2 | a contraction, for example `don't` |
| 6.3 | a sentence of more than 25 words |

Each rule has a severity: `error`, `warning` or `off`. The exit code is 1 when there is an error. The profile sets the severity of all rules, and `[rules]` changes one rule:

```toml
profile = "strict"      # "basic" (the default) checks the form of the writing:
                        # 1.1 and 1.6 are off, 6.3 is a warning.
                        # "strict" makes each finding an error.
[rules]
"4.2" = "off"

[languages.markdown.rules]
"1.6" = "off"           # no unknown-word findings in Markdown files
```

`[scopes.<name>]` gives a profile, rules and words to the paths it names, for example a strict `docs/` in a basic project. In a scope, `check = true` or `false` turns memelint on or off in those paths.

## Languages

memelint lints all languages that have a [tree-sitter grammar](https://tree-sitter.github.io/tree-sitter/#parsers). Rust, Python, JavaScript, TypeScript, Go, C, Java, Nix, TOML and Markdown are compiled in. The Nix package has each grammar that nixpkgs has. Other installations load `<name>.so` grammar files, the files that Neovim uses, from a directory. Markdown is off until `[languages.markdown]`, a scope or the top level has `check = true`.

## Your words

Technical names from your domain are not in the dictionary. memelint has lists of the technical names of software and of each language. Put the words of your project in `memelint.toml` at the project root, and examine that file as you examine code:

```toml
words = ["gantry", "sprocket"]
word-lists = ["words.txt", "https://example.com/team-words.txt"]

[languages.rust]              # for the files of one language
words = ["monomorphize"]
```

A word list has one word on each line. The list of a project becomes long; the [`memelint.toml`](memelint.toml) of this repository is an example.

[The bundled word lists](crates/memelint/words/) are not complete, and your aid is necessary. When you add a word of your language or of software to your list, send it in a pull request.

## Development

With Nix and direnv:

```bash
cp .envrc.template .envrc
direnv allow
```

Without direnv:

```bash
nix develop
```

Without flakes:

```bash
nix-shell
```

`task --list` shows the development commands.

## License

MIT or Apache-2.0, as you select. This applies to the code, the tests and this document. It does not apply to the vocabulary file, which comes from a document of ASD that we do not supply.

memelint is a project of the [Antimemetics Institute](https://antimemetics.institute).
