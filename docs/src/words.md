# Words

The ASD-STE100 dictionary has approximately 900 words. The technical names and technical verbs of your domain are not in it, and the standard lets you use them. Tell memelint which they are:

- `words`: a list in `memelint.toml`.
- `word-lists`: files with one word on each line and `#` comments. An entry is a path from the directory of `memelint.toml`, a URL, or the name of a bundled list.
- `default-word-lists`: the bundled lists are on unless this is `false`. The `software` list applies to all files, and the list of a language to its files.

The entries add to each other. memelint accepts a listed word with its usual inflections: the plural, `-ed` and `-ing`.

`memelint fetch` downloads the URLs into `~/.cache/memelint/words/`, and `memelint check` reads them from there.

The bundled lists are `software`, `c`, `cpp`, `go`, `java`, `javascript`, `markdown`, `nix`, `python`, `rust`, `toml`, `tsx` and `typescript`. [Their text](https://github.com/antimemes/memelint/tree/main/crates/memelint/words) is in the repository. They are not complete. If you add words, send a pull request.

To use only your lists:

```toml
default-word-lists = false
word-lists = ["words.txt"]
```

### The test for a bundled list

A bundled list applies to all projects of a language, and the `software` list to all projects. Thus a word goes into a bundled list when it is correct in these points for a person who does not know the project:

1. The word names one specified item, and it is the most accurate word for that item. `artifact`, `helper` and `context` are not correct. The sentence is better with the accurate word, for example "the log file" and not "the `artifact`".
2. The meaning comes from the language, its standard library, the system, the tools, or the general vocabulary of software. Not from one library, framework or domain. `schema`, `model` and `session` are not correct. Each library that a project uses gives the word a different meaning, and only the project knows its libraries.
3. In the comments of that language, most uses of the word have that one meaning. The other meanings are easy to see from the spelling, or from the subsequent word. `path` is correct: "file path" and "URL path" are far apart, and the sentence shows which one applies. `node` is correct: the runtime is written `Node.js`. `tag` is not correct. In comments it is a `git` tag, a markup tag, a struct tag and a build tag, with no mark that shows which.
4. ASD-STE100 has no approved word for that item, and the word is not an approved word. If it is, the list does not get the word.

Write the meaning after the word as a comment. Then the person who examines the pull request sees the meaning on the same line, and sees a different meaning immediately:

```text
sentinel      # a value that marks an end or an absence
node          # a point of a tree, a graph or a document; the runtime is Node.js
```

We examined these points against 566,000 comments of Python, Rust, Go and JavaScript projects. The words that a person reads incorrectly are the words of point 1 and point 2. These are words that many different items can be, for example `schema`, `session` and `resource`. Each library fills them with a different item. Words with meanings that are far apart, for example `key`, `path` and `union`, a person reads correctly.

A word that is not correct in one point can be a technical name in one project. Put it in the `words` of that project, not in a bundled list.

## Words of one directory

In a repository with many packages, each directory can have its words. Name the files with a glob:

```toml
subtree-word-lists = ["experiments/*/memelint-words.txt"]
```

An example file, `experiments/rotor-sim/memelint-words.txt`:

```text
# words of the rotor simulation
flywheel
torque-ramp
swashplate
```

The words of a file apply to the directory of the file and to all directories below it. A glob with `*` or `**` can find no file, and that is not an error. A path without `*` or `**` must be a file, as with `word-lists`. A path that ends with `/` is an error: the key names files, not directories. You select the file name: memelint has no default name, and it reads only the files you name.
