# memelint

Simplified Technical English lint for code comments and prose. memelint reads the comments and docstrings in your source files and tells you which words and sentences do not follow ASD-STE100. Markdown files are read as prose.

```console
$ memelint check
foo.py:2:29: 'ensure' is not approved; use 'make sure' [STE 1.1]
foo.py:3:7: 'don't' is a contraction; write the words in full [STE 4.2]
foo.py:3:13: 'delve' is not in the STE dictionary; if it is a technical name, add it to memelint.toml [STE 1.6]
foo.py:4:7: sentence has 28 words; the STE limit for descriptive text is 25 [STE 6.3]
foo.py:4:7: 'closing' is the -ing form of 'close'; STE allows only the infinitive, imperative, past tense and past participle [STE 3.5]
```

## Usage

[Nix](https://nixos.org/download/) is necessary. This builds the checker with the ASD-STE100 vocabulary and with each tree-sitter grammar that nixpkgs has, and runs it:

```bash
$(nix-build --no-out-link https://github.com/antimemes/memelint/archive/main.tar.gz)/bin/memelint check
```

With flakes:

```bash
nix run github:antimemes/memelint -- check
```

## Rules

ASD-STE100 Issue 9 has 53 rules. memelint checks the mechanical rules: the vocabulary, words that are approved in one part of speech only, contractions, `-ing` verb forms, and sentence length. The table below gives the status of each rule, and the findings give the rule ids The table comes from `crates/memelint/src/rules.rs`.

| Rule | Section | Rule (paraphrased) | memelint does | feasibility | memelint finding | Note |
|---|---|---|---|---|---|---|
| 1.1 | 1 Words | Use only approved words, technical nouns and technical verbs | yes | mechanical | unapproved word | dictionary plus the project allowlist |
| 1.2 | 1 Words | Use approved words only as their listed part of speech | in part | heuristic | approved only as | a rule with cue words. No finding when the context is not clear. A tagger can do more |
| 1.3 | 1 Words | Use approved words only in their approved meaning | no | judgment |  | the dictionary has the meanings. No program can apply them |
| 1.4 | 1 Words | Use only the listed forms of verbs and adjectives | yes | mechanical | -ing form | forms list |
| 1.5 | 1 Words | Technical nouns are approved by category | yes | mechanical |  | this is the allowlist |
| 1.6 | 1 Words | An unapproved word only when it is a technical noun | yes | mechanical | not in the STE dictionary | unknown words point at the allowlist |
| 1.7 | 1 Words | Do not use technical nouns as verbs | no | heuristic |  | allowlisted word in verb position |
| 1.8 | 1 Words | Use technical nouns that your organisation accepts | yes | mechanical |  | the allowlist is examined in the repository |
| 1.9 | 1 Words | Select short, clear technical nouns | no | judgment |  |  |
| 1.10 | 1 Words | No slang, regional or jargon technical nouns | no | judgment |  |  |
| 1.11 | 1 Words | One technical noun for each item | no | judgment |  | to find synonyms is not mechanical |
| 1.12 | 1 Words | Technical verbs are approved by category | yes | mechanical |  | allowlist |
| 1.13 | 1 Words | Do not use technical verbs as nouns | no | heuristic |  | the same as 1.7, for verbs |
| 1.14 | 1 Words | American spelling | no | mechanical |  | a British-to-American substitution table |
| 2.1 | 2 Multi-word nouns | Multi-word nouns of a maximum of 3 words | no | heuristic |  | a sequence of 4 dictionary nouns or allowlisted words |
| 2.2 | 2 Multi-word nouns | Write longer technical nouns in full | no | judgment |  |  |
| 3.1 | 3 Verbs | Only the verb forms in the dictionary | yes | mechanical | -ing form |  |
| 3.2 | 3 Verbs | Only the infinitive, imperative, simple present, simple past, simple future and past participle as adjective | no | heuristic |  | find perfect and progressive constructions |
| 3.3 | 3 Verbs | Past participle only as an adjective | no | heuristic |  | overlaps 3.6 |
| 3.4 | 3 Verbs | No complex auxiliary constructions | no | heuristic |  | have, has or had plus a participle. Modal verbs in sequence |
| 3.5 | 3 Verbs | -ing only in technical nouns | yes | mechanical | -ing form | the allowlist includes the exception |
| 3.6 | 3 Verbs | Active voice. Passive only when the agent is unknown | no | heuristic |  | be plus the participle of an approved verb. 'by' plus an agent is the strong signal |
| 3.7 | 3 Verbs | Give an action with a verb, not with a noun | no | heuristic |  | make or do plus a noun with the stem of an approved verb |
| 4.1 | 4 Sentences | Short, clear sentences | in part | heuristic | sentence has | length is mechanical, clarity is judgment |
| 4.2 | 4 Sentences | No words removed, no contractions | in part | heuristic | contraction | contractions are mechanical. For articles that are not there, see 4.5 |
| 4.3 | 4 Sentences | Vertical lists for complex text | no | judgment |  |  |
| 4.4 | 4 Sentences | Connections between related sentences | no | judgment |  |  |
| 4.5 | 4 Sentences | Use articles and demonstratives before nouns | no | heuristic |  | a singular noun with no article after a verb. Many incorrect findings in short comments |
| 5.1 | 5 Procedural writing | Procedural sentences of at most 20 words | no | mechanical |  | sentences that start with an approved verb are procedural |
| 5.2 | 5 Procedural writing | One instruction in each sentence | no | heuristic |  | 2 imperatives with 'and' between them |
| 5.3 | 5 Procedural writing | Instructions in the imperative | no | heuristic |  | 'you must' plus a verb points to the imperative |
| 5.4 | 5 Procedural writing | Condition before the instruction | no | heuristic |  | an imperative with an 'if' clause at the end |
| 5.5 | 5 Procedural writing | Notes give information, not instructions | no | judgment |  |  |
| 6.1 | 6 Descriptive writing | Give information gradually | no | judgment |  | Congree does it as some smaller rules |
| 6.2 | 6 Descriptive writing | Key words give the text a structure | no | judgment |  |  |
| 6.3 | 6 Descriptive writing | Descriptive sentences of at most 25 words | yes | mechanical | sentence has |  |
| 6.4 | 6 Descriptive writing | Paragraphs group related information | no | judgment |  |  |
| 6.5 | 6 Descriptive writing | One topic for each paragraph | no | judgment |  |  |
| 6.6 | 6 Descriptive writing | A maximum of 6 sentences in each paragraph | no | mechanical |  | a comment block is a paragraph |
| 7.1 | 7 Safety instructions | Identify the level of risk with warning or caution | no | n/a |  | safety text does not occur in code comments |
| 7.2 | 7 Safety instructions | Start a safety instruction with a command or condition | no | n/a |  |  |
| 7.3 | 7 Safety instructions | Tell the risk | no | n/a |  |  |
| 8.1 | 8 Punctuation and word count | Standard punctuation, but no semicolons | no | mechanical |  | must not give findings for code fragments |
| 8.2 | 8 Punctuation and word count | Hyphenate directly related words | no | judgment |  |  |
| 8.3 | 8 Punctuation and word count | Parentheses only for the uses in the list | no | judgment |  |  |
| 8.4 | 8 Punctuation and word count | In a vertical list a colon ends the word count | no | mechanical |  | word-count rule |
| 8.5 | 8 Punctuation and word count | Text in parentheses counts as one word | no | mechanical |  | word-count rule |
| 8.6 | 8 Punctuation and word count | Numbers, units, abbreviations, identifiers, quoted text, titles and proper nouns count as one word | no | mechanical |  | word-count rule |
| 8.7 | 8 Punctuation and word count | Hyphenated words count as one word | yes | mechanical |  | tokens split at whitespace |
| 9.1 | 9 Writing practices | Write the sentence again when word-for-word substitution is not possible | no | judgment |  |  |
| 9.2 | 9 Writing practices | Use each approved word correctly | no | judgment |  |  |
| 9.3 | 9 Writing practices | No phrasal verbs | no | mechanical |  | an approved verb plus a particle, when the pair is not an approved entry |
| 9.4 | 9 Writing practices | Consistent terminology and style | no | judgment |  | in part a consistency check for each project |

53 rules. memelint does: 10 yes, 3 in part, 40 no. Feasibility: 18 mechanical, 15 heuristic, 17 judgment, 3 n/a.

## Languages

memelint lints all languages that have a [tree-sitter grammar](https://tree-sitter.github.io/tree-sitter/#parsers). Rust, Python, JavaScript, TypeScript, Go, C, Java, Nix, TOML and Markdown are compiled in. For each other language, memelint loads `<name>.so` from a grammar directory: `--grammars`, `MEMELINT_GRAMMARS` or `~/.local/share/memelint/grammars`. These are the tree-sitter grammar files that Neovim and nixpkgs use.

## Your words

Technical names from your domain are not in the dictionary. memelint has lists of the technical names of software and of each language, and uses them unless you set `default-word-lists = false`. Put the names of your project in `memelint.toml` at the project root, and examine that file as you examine code:

```toml
words = ["gantry", "sprocket"]
word-lists = ["words.txt", "https://example.com/team-words.txt"]

[languages.rust]              # for the files of one language
words = ["monomorphize"]
```

A word list has one word on each line. `memelint fetch` downloads the URLs into the cache. The bundled lists are in `crates/memelint/words/`.

A long list can go in a different file with `words-file = "words.txt"`. Paths in `ignore = ["vendor/"]` are not examined, with the syntax of `.gitignore`.

## The dictionary

ASD-STE100 is free to download, but it has copyright. Thus the memelint binary does not contain the dictionary. The Nix package makes the vocabulary from the PDF on your machine. With a different installation, make it one time on each machine:

```bash
memelint-import asd-ste100 --pdf ASD-STE100_ISSUE9.pdf
```

This writes the vocabulary to `~/.cache/memelint/`, which is not in your projects. `--vocab` or `MEMELINT_VOCAB` point the checker to a different file.

## Development

`nix develop` gives you the toolchain, the PDF and all grammars. The workspace has 3 crates: `memelint` (the checker), `memelint-import` (the importer) and `memelint-common` (the vocabulary file and its location). The end-to-end tests in `crates/memelint/tests/cases/` compare the output of `memelint check` on fixture worktrees with `expected.txt`. `UPDATE_EXPECT=1 cargo test --test cases` writes the correct output again after a change.

## License

MIT or Apache-2.0, as you select. This applies to the code, the tests and this document, and not to the vocabulary file, which comes from a document of ASD.
