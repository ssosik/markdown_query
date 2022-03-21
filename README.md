# MarkdownQuery: mdq

A local text file Xapian indexer and query tool. Started life as a minimal
[Zettelkasten](https://zettelkasten.de/posts/overview/#principles)-like
Markdown+FrontMatter document indexer and query interface: Keep lots of small
notes and then quickly find them again using Information Retrival-style natural
language queries.

I would like to extend this to index other bodies of text on a local filesystem,
such as man/info pages. TODO

# Example

For demonstration purposes, here I indexed a bunch of articles from Wikipedia.
[![asciicast](https://asciinema.org/a/435930.png)](https://asciinema.org/a/435930)


# About

How do you do personal notes? Grepping through a folder full of notes isn't the
most effective approach; using Xapian is a better way.

I originally implemented this as a Vim plugin using some Python code built
around [Xapian](https://xapian.org/), called
[tika](https://github.com/ssosik/tika). I decided to rewrite this in Rust, and
after exploring other options for Information Retrieval based search tools like
Tantivy and Fuzzy-Finder, I went back to Xapian: it's the best.

# Note

This is my first Rust project, I welcome any comments/PRs/issues related to
improving the code here.

There's still a lot I'd like to do here, but this version should at least be
usable.

# Wishlist of TODOs

* [x] fix preview output
* [ ] use pest for query parsing
* [x] Colorize preview
* [ ] Support platforms besides linux, mac
* [ ] Delete entries from local cache
* [ ] Query/filter on time range
* [ ] Vim Plugin
* [ ] TUI start list at the bottom instead of the top
* [ ] TUI select many
* [ ] searching/jumping-to/highlighting in preview
* [ ] pageup/down; ctrl-w
* [ ] Cache MD5 hashes of files using [kv](https://docs.rs/kv/0.22.0/kv/) to
    skip indexing unchanged files
* [ ] Keep track of document access count in KV and use that as a weighting
    factor in query results
* [ ] Keep track of all Tags to be used for autocompletion
* [x] cleanups, refactoring, rust-analyze, clippy and linting
* [ ] Optimize binary and compile times
  * [ ] https://matklad.github.io//2021/09/04/fast-rust-builds.html
  * [ ] https://pingcap.com/blog/rust-compilation-model-calamity
  * [ ] https://doc.rust-lang.org/rustc/profile-guided-optimization.html
* Prune dependencies: https://pingcap.com/blog/rust-compilation-model-calamity#recent-work-on-rust-compile-times
* [x] CLI option for passing in starting query for interactive mode
* [ ] CLI option to emit JSON instead of filename
* [ ] import man/info pages and other canonical documentation for indexing and IR
* [ ] Add URL tag, support multiple?
* [x] Support multiple Author tags
* [x] Add tests
* [x] Query/filter on tags (make sure this is working properly)
* [x] statically link xapian-core

# Installation

For now, clone this repo and run the makefile.

# Usage

```
# Index a source directory, note the single quotes used here to prevent the
# shell from expanding the wildcard here
mdq [db dir] update '/path/to/markdown-directory'

# Run an interactive query against an index
mdq [db dir]
```

# Note on Markdown+Frontmatter format

I would like to make this pluggable, but for now it's hardcoded to look for
Markdown+Frontmatter files like this:

    ---
    author: Steve Sosik
    date: 2021-01-15T08:23:24-05:00
    tags:
    - vim
    title: How to grep open buffers in Vim
    ---
    
    Run this command
    
    ```
    :bufdo vimgrepadd [search] % | copen
    ```

# Requirements

lightly patched version of xapian-rusty, included here as a submodule.

zlib and xapian-core, which are bundled here.

To get started in a ubuntu-18.04 docker image:
```bash
apt-get update --yes
apt-get upgrade --yes
apt-get install --yes build-essential git
git clone --recurse-submodules https://github.com/ssosik/xapiary.git
cd xapiary
make
```

# Development

Any modern standard Rust installation should probably work.

I use [NixOS](https://nixos.org/) along with [Direnv](https://direnv.net/) and [Direnv Nix Integration](https://github.com/direnv/direnv/wiki/Nix)

```bash
git clone --recurse-submodules git@github.com:ssosik/xapiary.git
cd xapiary
direnv allow
# Wait some time for Nix to install all of the Rust tooling
rustup install stable

make test
make run

# hack, hack, hack
```
