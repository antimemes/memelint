# Configuration

memelint reads `memelint.toml` at the project root. Each key is optional. The first line gives your editor the schema:

```toml
#:schema https://raw.githubusercontent.com/antimemes/memelint/main/crates/memelint/memelint.schema.json

profile = "strict"                 # "strict" or "basic"
words = ["gantry", "sprocket"]
word-lists = ["words.txt", "https://example.com/team-words.txt"]
default-word-lists = true
ignore = ["vendor/", "*.generated.go"]

[rules]
"6.3" = "warning"                  # "error", "warning" or "off"

[languages.rust]                   # for the files of one language
words = ["monomorphize"]
word-lists = ["rust-words.txt"]
default-word-lists = true

[languages.rust.rules]
"1.6" = "off"
```

| Key | Effect |
|---|---|
| `profile` | the severity of each rule, see [Rules](rules.md) |
| `rules` | the severity of one rule |
| `words` | the technical names and verbs of the project, see [Words](words.md) |
| `word-lists` | files, URLs and bundled lists of words |
| `default-word-lists` | use the bundled lists (default: `true`) |
| `ignore` | paths that memelint does not examine, in the syntax of `.gitignore` |
| `languages.<name>` | the same keys for the files of one language, see [Languages](languages.md) |
