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

A word goes into a bundled list when it is correct in each of these points:

1. It is the name of one item in software, for example `mutex` or `stdin`. The person who reads a comment does not think about which meaning applies.
2. ASD-STE100 has no approved word for that item. If it has one, the approved word is correct, and the list does not get the word.
3. It is not also a word of general English with a different, less clear meaning. `artifact` names a build output, but in a sentence it can mean many different items. STE removes words of this type. Write the accurate words: the log file, the test report, the compiled program.

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
