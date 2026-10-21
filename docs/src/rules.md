# Rules

ASD-STE100 Issue 9 has 53 rules. A program can check a part of them. memelint checks these:

| Rule | Finding |
|---|---|
| 1.1 | a word that is not approved, with the approved alternatives, for example `commence` (use `start`) |
| 1.2 | an approved word in a part of speech in which it is not approved. For example, `test` is a noun, thus "do a test" and not "test it" |
| 1.6 | a word that is not in the dictionary and not in your word list, for example `kubelet`. Put the words of your project in `memelint.toml`, and they are approved |
| 3.5 | the `-ing` form of an approved verb, for example `starting` |
| 4.2 | a contraction, for example `don't` |
| 6.3 | a sentence of more than 25 words |

Each rule has a severity: `error`, `warning` or `off`. The exit code is 1 when there is an error. The profile sets the severity of each rule:

| Profile | Effect |
|---|---|
| `basic` (the default) | 1.1 and 1.6 are off, 6.3 is a warning |
| `strict` | each finding is an error |

The basic profile checks the form of the writing, and no word list is necessary. The strict profile also checks the vocabulary, with the word list of the project.

`[rules]` changes one rule, and `[languages.<name>.rules]` changes it for the files of one language. The language value applies first, then `[rules]`, then the profile:

```toml
profile = "strict"

[rules]
"4.2" = "off"

[languages.markdown.rules]
"1.6" = "off"
```

A scope names paths and gives them a profile and rules. Thus a project that is `basic` at the root can make `docs/` strict:

```toml
[scopes.docs]
paths = ["docs/"]       # as in .gitignore, from the directory of memelint.toml
profile = "strict"

[scopes.docs.rules]
"6.3" = "off"
```

In these paths, the scope rules apply first, then `[languages.<name>.rules]`, then `[rules]`, then the scope profile, then the top-level profile. When two scopes name one file, the last of them in `memelint.toml` sets the severity, and the words of the two apply.

A scope with `check = true` or `check = false` turns memelint on or off in its paths. The `check` of the last scope applies first, then the `check` of the language, then the top-level `check`.

A rule that gives no findings cannot be set. memelint stops with an error message when it reads `memelint.toml`.
