# Font research repository

The extraction library, font-specific printer examples, Python reconstruction and
fitting tools, research notes and collected experiments now live in the private
[codyps/zpl-font-extract](https://github.com/codyps/zpl-font-extract) repository.
It is not a member, dependency or submodule of this workspace. Public builds,
CI, Nix packaging and releases require no access to that repository.

The private repository preserves path-filtered Git history, the complete current
research corpus (6,870 files, including previously uncommitted files), original source hashes,
and the pre-migration worktree patch. Every file from the ignored `_font-work`
tree is also preserved in checksummed private Release archives, including caches
and unique working results: 1,125,093 files and 20,869,837,315 original bytes.
The [preservation release](https://github.com/codyps/zpl-font-extract/releases/tag/migration-2026-10-05)
includes the archive, per-file inventory and frozen research executable.
Its `migration/` records identify and verify both
the archived import and the large working-data archive.

ZPL retains the runtime bitmap types/decoder, embedded font assets, original
TrueType renderer, and existing printer-accuracy regressions. The TrueType
integration tests use a fixed 488-file, 4,783,744-byte corpus under
`zpl/tests/fixtures/truetype-regression`. Those bytes and all existing assertions
are unchanged; `sources.json` pins each file to its private-repository source.
New font experiments must not be written into this runtime regression directory.
The resident-font suite also retains 52 existing request/PNG files (50,236 bytes)
under its own `font-study/` subset. Their original hashes and assertions are
unchanged. The shared SHA-256 test helper lives in `zpl/tests/support/digest.rs`.

To work on font extraction, clone the private repository separately and follow
its README. It contains its own Cargo workspace, lockfile and offline checks.
The renderer revision used by the experiments was local-only at the split, so
a small, checksummed runtime source snapshot is preserved there for reproduction.
Renderer development continues here; refresh that dependency snapshot explicitly.

This migration removes research from the current ZPL tree. It does not rewrite
existing public Git history or make already-published history private. Historical
clone size is unchanged; rewriting shared history is a separate operation.

## Migration validation

The private checkout passed 62 Rust and 152 Python tests; its original corpus
was verified again from a fresh GitHub clone. Every working-archive file was
read back and matched its SHA-256, and GitHub's stored asset digests matched
the local compressed files. The private repository visibility was verified.

After the split, all 668 ZPL workspace tests and seven capture-tool tests pass.
The workspace test command is `cargo test --locked --workspace --all-targets`;
the validation shell supplied SQLite from the declared Nix development package.
Rust formatting, TOML formatting and whitespace checks pass. All 540 retained
request/font/reference files match their original hashes. Nix package evaluation
is checked separately; it is not a Nix package build or VM test.
