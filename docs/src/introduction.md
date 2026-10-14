# memelint

memelint is a lint for Simplified Technical English (ASD-STE100). It reads the comments and docstrings in source files, and Markdown files as prose. It tells you which words and sentences do not follow the standard.

Run it at the root of a project. [Nix](https://nixos.org/download/) is necessary:

```bash
nix run github:antimemes/memelint -- check
```

Each finding gives the file, the position, the severity, the message and the rule:

```console
foo.py:2:29: error: 'ensure' is not approved; use 'make sure' [STE 1.1]
```

The exit code is 1 when there is an error.

## The dictionary

ASD-STE100 has copyright, thus the binary does not contain its dictionary. The Nix package makes the vocabulary from the PDF on your machine. With a different installation, make it one time:

```bash
memelint-import asd-ste100 --pdf ASD-STE100_ISSUE9.pdf
```

This writes `~/.cache/memelint/asd-ste100.json`. `--vocab` or `MEMELINT_VOCAB` point the checker to a different file.
