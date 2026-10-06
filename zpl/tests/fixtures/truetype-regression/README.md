# Fixed TrueType runtime regression corpus

These exact bytes are the subset of the private [font research repository](https://github.com/codyps/zpl-font-extract) consumed by `zpl/tests/truetype.rs`. `sources.json` records their original research-relative paths, sizes and SHA-256 hashes.

This is a bounded runtime test corpus, not an experiment output directory. New captures, optimizer states and research candidates belong in the private repository. Refresh individual regression witnesses deliberately when a renderer change needs them; do not mirror the research corpus.

The existing independent FreeType references and ZD621 203 DPI, V93.21.33Z native preview assertions are unchanged. Per-campaign manifests and capture records preserve model, identity, controls and hashes.
