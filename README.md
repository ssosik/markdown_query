# tika

Things I Know About - A Minimal [Zettelkasten](https://zettelkasten.de/posts/overview/#principles)-inspired Markdown+FrontMatter document indexer and query interface.

Keep lots of small notes and then quickly find them again using Information
Retrival-style natural language queries.

# About

I originally implemented this as a Vim plugin using some Python code built
around [Xapian](https://xapian.org/).

# Note

This is my first Rust project, the code here is probably not ideal.

# TODOs

* [x] Add tests
* [ ] Delete entries from local cache
* [ ] Query/filter on time range
* [ ] timestamp ranges
* [x] Query/filter on tags (make sure this is working properly)
* [ ] Update README
* [ ] Vim Plugin
* [ ] TUI select many
* [ ] searching/jumping-to/highlighting in preview
* [ ] pageup/down; ctrl-w
* [ ] TUI start list at the bottom instead of the top
* [ ] Cache MD5 hashes of files using `kv` to skip indexing unchanged files
* [ ] Keep track of access count in KV
* [ ] Keep track of all Tags used for autocompletion
* [ ] cleanups, refactoring, rust-analyze, clippy and linting
* [ ] statically link xapian-core
* [ ] fix all gratuitus allows and unused imports
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
DYLD_LIBRARY_PATH=xapian-core-1.4.17/.libs/ ./target/debug/tika -i

# Run a query against an index
./target/debug/tika
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
