# Configuration

memelint reads `memelint.toml` at the project root. Each key is optional. The first line gives your editor the schema:

```toml
#:schema https://raw.githubusercontent.com/antimemes/memelint/main/crates/memelint/memelint.schema.json

profile = "basic"                  # "basic" (the default) or "strict"
words = ["gantry", "sprocket"]
word-lists = ["words.txt", "https://example.com/team-words.txt"]
subtree-word-lists = ["experiments/*/memelint-words.txt"]
default-word-lists = true
ignore = ["vendor/", "*.generated.go"]

[rules]
"6.3" = "warning"                  # "error", "warning" or "off"

[languages.markdown]               # for the files of one language
check = true                       # examine these files (default: true for code, false for prose)
words = ["spread"]
word-lists = ["doc-words.txt"]
default-word-lists = true

[languages.markdown.rules]
"1.6" = "off"
```

| Key | Effect |
|---|---|
| `profile` | the severity of each rule, see [Rules](rules.md) |
| `rules` | the severity of one rule |
| `words` | the technical names and verbs of the project, see [Words](words.md) |
| `word-lists` | files, URLs and bundled lists of words |
| `subtree-word-lists` | globs of word files, each for the directory of the file and all directories below it, see [Words](words.md) |
| `default-word-lists` | use the bundled lists (default: `true`) |
| `ignore` | paths that memelint does not examine, in the syntax of `.gitignore` |
| `languages.<name>` | the same keys for the files of one language, see [Languages](languages.md) |
| `languages.<name>.check` | examine the files of one language (default: `true` for code, `false` for prose) |

Markdown files are prose. memelint examines them only when `[languages.markdown]` has `check = true`.
