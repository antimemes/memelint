# Output format

`memelint check` writes each finding on stdout. The summary and the notes stay on stderr. `--format` sets the form of the findings:

| Key | Effect |
|---|---|
| `text` (the default) | one line for each finding: `file:line:col: severity: message [STE id]` |
| `json` | JSON text with the counts and the findings |
| `sarif` | SARIF 2.1.0 |

```bash
memelint check --format json
memelint check --format sarif > memelint.sarif
```

The JSON text has `issue`, `files`, `comments`, `errors`, `warnings` and `findings`. Each finding has `path`, `line`, `column`, `severity`, `rule`, `message`, and `fix` when memelint has a replacement. `fix` has `start`, `end`, `replacement` and `safe`.

The SARIF file has one run. The tool name is memelint. Each item is one finding. The rule id is the STE id, for example `1.1`. GitHub shows these findings on the pull request.

The GitHub Action puts `memelint` on `PATH` and then runs `check`. A step after it can write SARIF to a file. GitHub then shows the findings:

```yaml
- uses: actions/checkout@v4
- uses: antimemes/memelint@latest
- run: memelint check --format sarif > memelint.sarif
- uses: github/codeql-action/upload-sarif@v3
  if: always()
  with:
    sarif_file: memelint.sarif
```

`--format` does not change the exit code. The exit code is 1 when there is an error.
