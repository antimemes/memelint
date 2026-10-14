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
| `strict` (the default) | each finding is an error |
| `basic` | 1.6 and 6.3 are warnings |

`[rules]` changes one rule, and `[languages.<name>.rules]` changes it for the files of one language. The language value applies first, then `[rules]`, then the profile:

```toml
profile = "basic"

[rules]
"4.2" = "off"

[languages.markdown.rules]
"1.6" = "off"
```

A rule that gives no findings cannot be set. memelint stops with an error message when it reads `memelint.toml`.
