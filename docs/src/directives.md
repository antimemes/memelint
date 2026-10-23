# Directives

A line `memelint: ignore` in a comment tells memelint not to give findings for that comment. The line is not prose.

```rust
//! memelint: ignore
//! Don't wait. This comment is a quote.
```

| Line | Effect |
|---|---|
| `memelint: ignore` | no findings for this comment |
| `memelint: ignore 4.2` | no findings of rule 4.2 for this comment |
| `memelint: ignore 4.2, 6.3` | no findings of those rules for this comment |
| `memelint: ignore-next` | no findings for the next comment |
| `memelint: ignore-file` | no findings for this file |

The line can have the comment marks of the language: `//`, `///`, `//!`, `#`, `/* ... */`. A Markdown HTML comment `<!-- memelint: ignore -->` applies to the next paragraph.

The rule id can be `4.2` or `STE 4.2`. Empty means all rules.

`--fix` does not change a finding that a directive turns off.
