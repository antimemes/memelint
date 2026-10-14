# memelint

memelint is a lint for Simplified Technical English (ASD-STE100). It reads the comments and docstrings in source files, and Markdown files as prose. It tells you which words and sentences do not follow the standard.

Run it at the root of a project. [Nix](https://nixos.org/download/) is necessary:

```bash
nix run github:antimemes/memelint -- check
```

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

ASD-STE100 has copyright, thus the binary does not contain its dictionary. The Nix package makes the vocabulary from the PDF on your machine.
