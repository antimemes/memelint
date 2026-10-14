# memelint

memelint lints the memes of a project: the words that spread through comments, docstrings, and documentation. It tells you which words and sentences do not follow Simplified Technical English (ASD-STE100), with an allowlist for the words of your project.

Each finding gives the file, the position, the severity, the message and the rule:

```console
$ memelint check
foo.py:2:29: error: 'ensure' is not approved; use 'make sure' [STE 1.1]
foo.py:3:7: error: 'don't' is a contraction; write the words in full [STE 4.2]
foo.py:3:13: error: 'delve' is not in the STE dictionary; if it is a technical name, add it to memelint.toml [STE 1.6]
foo.py:4:7: error: sentence has 28 words; the STE limit for descriptive text is 25 [STE 6.3]
foo.py:4:7: error: 'closing' is the -ing form of 'close'; STE allows only the infinitive, imperative, past tense and past participle [STE 3.5]
```

The exit code is 1 when there is an error.

## Installing

ASD-STE100 is copyrighted, thus the binary does not contain its dictionary. The first run downloads the PDF from ASD and makes the vocabulary in `~/.cache/memelint/`. The Nix package makes it when it builds.

### On your machine

Use the install script, or use Nix.

#### The install script

The release binaries are for Linux and macOS, on x86_64 and aarch64:

```bash
curl -fsSL https://memelint.sh | sh
memelint check
```

#### Nix

With [Nix](https://nixos.org/download/), there is no installation. This builds the checker with the vocabulary and with each tree-sitter grammar that nixpkgs has, and runs it:

```bash
$(nix-build --no-out-link \
  https://github.com/antimemes/memelint/archive/main.tar.gz)\
/bin/memelint check
```

With flakes:

```bash
nix run github:antimemes/memelint -- check
```

### In GitHub Actions

Put this in `.github/workflows/memelint.yml`. The findings become annotations on the pull request:

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
