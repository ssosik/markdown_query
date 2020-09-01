# zkfm
Minimal Zettelkasten-inspired Markdown+FrontMatter document indexer and query interface

# Usage

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
