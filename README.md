# Xapian-Query-CLI: xq

A local text file Xapian indexer and query tool. Original effort was focused on
building a minimal
[Zettelkasten](https://zettelkasten.de/posts/overview/#principles)-like
Markdown+FrontMatter document indexer and query interface: Keep lots of small
notes and then quickly find them again using Information Retrival-style natural
language queries.

Inspired by [vimwiki](https://github.com/vimwiki/vimwiki).

After being focused on my personal notes in Markdown+Frontmatter format, I
realized this could be useful for indexing other bodies of text on a local
filesystem, such as man/info pages. TODO

# About

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

# TODOs

* [x] Add tests
* [x] Query/filter on tags (make sure this is working properly)
* [x] statically link xapian-core
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
* [ ] cleanups, refactoring, rust-analyze, clippy and linting
* [ ] CLI option for passing in starting query for interactive mode
* [ ] CLI option to emit JSON instead of filename
* [ ] import man/info pages and other canonical documentation for indexing and IR
* [ ] Add URL tag, support multiple?
* [ ] Support multiple Author tags

# Installation

TODO

# Usage

TODO update

```
# Index a source directory
xq -i

# Run a query against an index
xq
```

# Requirements

lightly patched version of xapian-rusty, included here as a submodule.

zlib and xapian-core, which are bundled here.

# Development

Any modern standard Rust installation should probably work.

I use [NixOS](https://nixos.org/) along with [Direnv](https://direnv.net/) and [Direnv Nix Integration](https://github.com/direnv/direnv/wiki/Nix)

```bash
git clone https://github.com/ssosik/tika
cd tika
direnv allow
# Wait some time for Nix to install all of the Rust tooling
rustup install stable

make test
make run

# hack, hack, hack
```
