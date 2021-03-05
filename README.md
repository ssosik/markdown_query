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

* [ ] Add tests
* [ ] Use [fuzzy_matcher](https://crates.io/crates/fuzzy-matcher)
* [ ] Delete entries from local cache
* [ ] Query/filter on time range
* [ ] Query/filter on tags (make sure this is working properly)
* [ ] Cleanup README
* [ ] Vim Plugin
* [ ] Point to original python+Xapian code

# Installation

TODO

# Usage

TODO update

```
# Index a source directory
./target/debug/zkfm index ~/workspace/vimdiary

# Run a query against an index
./target/debug/zkfm query vault
```

OLD
```
sk --preview='bat --color=always ~/workspace/vimdiary/{}' --ansi -i -c './target/debug/tika -s ~/workspace/vimdiary -q "{}" | jq -r .filename\[0\]'
```

# Development

Any modern standard Rust installation should probably work.

I use [NixOS](https://nixos.org/) along with [Direnv](https://direnv.net/) and [Direnv Nix Integration](https://github.com/direnv/direnv/wiki/Nix)

```bash
git clone https://github.com/ssosik/tika
cd tika
direnv allow
# Wait some time for Nix to install all of the Rust tooling
rustup install stable

cargo build

# hack, hack, hack
```
