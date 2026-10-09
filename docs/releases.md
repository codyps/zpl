# Releases

The [Release-plz workflow](../.github/workflows/release-plz.yml) runs on pushes to
`main`, or manually from the Actions tab on `main`. Its publishing stages are:

1. `release` publishes prepared versions to **crates.io**, pushes package tags,
   and creates GitHub releases with changelog notes. With
   `release_always = false`, release-plz does this only when the checked-out
   commit is associated with a PR whose branch starts with `release-plz-`.
   Ordinary feature and maintenance merges skip publication.
2. `npm-release` publishes `@codyps/zpl` after a completed stable `zpl` release
   at the same commit. It builds and tests the Wasm package and uses the Rust
   renderer version as the npm version. Ordinary commits and releases of only
   other crates skip npm publication.
3. `python-version` selects the same completed stable `zpl` release. `python-build`
   builds and tests native wheels and a source distribution, and `python-release`
   publishes `zplkit` to **PyPI** with the renderer version.
4. `elixir-version` selects the same completed stable renderer release. `elixir-build`
   builds and tests a portable source archive, and `elixir-release` publishes
   `zpl` to **Hex** with the renderer version.
5. After all four registries succeed, `release-pr` creates or updates a PR containing
   package versions, changelogs, and workspace dependency updates, using the
   published crates.io versions as the baseline. A failed publication stops
   release preparation so it cannot race ahead of the registry.

`raster-diff`, `zpl-bitmap-fonts`, and `zpl` are publishable and managed by
release-plz. The empty `zplkit` 0.0.0 placeholder is published separately and
explicitly excluded from release-plz; it does not implement the Python bindings.
The remaining workspace packages set `publish = false`. Release-plz
derives dependency order and publishes the raster and bitmap-font dependencies
before `zpl`. Tags and GitHub releases use `<crate>-v<version>`.

The action and Rust/checkout actions are pinned to commit SHAs; the release-plz
binary is pinned separately. Publishing jobs are serialized, and an active publish
is not cancelled by newer pushes; release-PR jobs are also serialized. Release jobs
are restricted to `codyps/zpl` on `main` and have bounded execution time.

CI excludes pushes to release-plz's temporary `release-plz-*-tmp-*` branches to
avoid duplicate builds and cancellation noise. The lasting release branch,
release PR, and `main` retain their normal CI checks.

## Trusted publishing setup

Configure a GitHub trusted publisher in the crates.io settings for
[`raster-diff`](https://crates.io/crates/raster-diff/settings),
[`zpl-bitmap-fonts`](https://crates.io/crates/zpl-bitmap-fonts/settings), and
[`zpl`](https://crates.io/crates/zpl/settings), with these exact values:

| Field | Value |
| --- | --- |
| Repository owner | `codyps` |
| Repository name | `zpl` |
| Workflow filename | `release-plz.yml` |
| Environment | Leave empty; the publishing job does not use an environment. |

The workflow filename is the file's basename, not its display name or its full
`.github/workflows/` path. If an environment restriction is added on crates.io,
the publishing job must declare the same GitHub environment. Renaming the workflow
file also requires updating both trusted-publisher entries.

The pinned release-plz binary supports native trusted publishing. The `release`
job grants `id-token: write`, and release-plz exchanges GitHub's OIDC identity for
a short-lived crates.io token when it has a crate to publish. It does not need
`rust-lang/crates-io-auth-action` or a `CARGO_REGISTRY_TOKEN` secret. Do not set an
empty token variable: leave it unset so release-plz can use OIDC. Both crates must
already exist on crates.io; trusted publishing cannot perform a first publication.

The publishing job uses the default `GITHUB_TOKEN` with `contents: write` to
create tags/releases and `pull-requests: read` to detect a release PR. The
`release-pr` job separately uses the `RELEASE_PLZ_TOKEN` repository secret so its
PR creation and updates trigger ordinary PR CI. Store an appropriately scoped
PAT or GitHub App token in that secret, following the
[token documentation](https://release-plz.dev/docs/github/token). It is a GitHub
credential, not a crates.io publishing token. Keep **Settings → Actions → General
→ Workflow permissions → Allow GitHub Actions to create and approve pull
requests** enabled; no auto-approval step is configured.

## npm publishing setup

The `npm-release` job in `release-plz.yml` uses npm trusted publishing on a
GitHub-hosted Ubuntu runner. No `NPM_TOKEN` or `NODE_AUTH_TOKEN` secret is needed.
It remains in the same workflow because tags and releases created with
`GITHUB_TOKEN` do not trigger another release workflow. See
[GitHub workflow triggering](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow).

Before the first automatic npm release:

1. Ensure your npm account owns or can publish under the `@codyps` scope, and
   enable npm two-factor authentication. The GitHub username does not reserve
   the corresponding npm scope.
2. Merge the Node package and publishing workflow into `main`.
3. Create the initial public npm package from the reviewed checkout, using the
   [Node build prerequisites](../zpl-node/README.md#build-and-install-from-this-checkout):

   ```sh
   npm login
   cd zpl-node
   npm run build
   npm test
   npm publish --access public
   ```

   This is the one-time manual publication using your interactive npm login.
   The initial package version is `0.1.0`; subsequent automatic releases take
   the version from the released `zpl` crate. Do not manually publish a future
   renderer version in advance of its release.
4. Open **npmjs.com → @codyps/zpl → Settings → Trusted publishing** and add a
   GitHub Actions publisher with these values:

   | Field | Value |
   | --- | --- |
   | Organization or user | `codyps` |
   | Repository | `zpl` |
   | Workflow filename | `release-plz.yml` |
   | Environment name | Leave empty. |
   | Allowed actions | Enable direct publishing with `npm publish`. |

   npm currently requires a new trusted-publisher configuration to complete
   its first successful publish within **2 days**. Configure it near the next
   release; if it expires, delete it and create it again. See the
   [npm trusted publishing setup and expiry rules](https://docs.npmjs.com/trusted-publishers/).

Then merge a release-plz PR containing a new stable `zpl` version. The workflow
checks that `zpl-v<version>` points to its exact checkout and has a completed,
non-prerelease GitHub release. It stamps that version into the npm manifest in
CI, builds Wasm, runs native and Node tests (including installing a packed
archive), and publishes the public package. npm supplies provenance automatically
for a public package built in a public repository via trusted publishing.
The checked-in npm version is a local-development version; CI does not commit
its version stamp back to the repository.

This policy couples npm releases to **stable `zpl` releases**, not every workspace
crate release. For a Node-wrapper-only fix, explicitly arrange a `zpl` version
bump in the release PR, including the normal Cargo lockfile/dependency updates;
release-plz may not infer a renderer release from files outside that crate.
Prereleases are not published to npm by this workflow.

On a retry, an already-published npm version is skipped. Registry errors other
than a missing package/version stop the job. If crates.io succeeded but npm
failed, fix the npm setup and use **Re-run failed jobs** on that original run;
this retains the release commit even if `main` has advanced. A fresh manual
run on a later ordinary commit does not backfill an older npm release.
A skipped existing version does not exercise or validate the OIDC configuration.

Verify the published result with:

```sh
npm view @codyps/zpl version dist-tags --registry=https://registry.npmjs.org/
```

The release selection tests and local dry runs do not verify npm ownership or
OIDC authentication. That requires an actual successful publish in Actions.

## PyPI publishing setup

The Python distribution is **`zplkit`**, imported as `zplkit`, and links this
repository's Rust `zpl` crate. Automatic releases use exactly the stable renderer
version, following the same `zpl-v<version>` tag and completed GitHub release
checks as npm. A shared selector prevents publishing on ordinary commits,
prereleases, missing GitHub releases, or tags pointing at a different commit.
Python-wrapper-only changes need an explicit renderer version bump in the release
PR, just like Node-wrapper-only changes. The binding's checked-in Cargo version is
for local development; CI stamps the binding manifest and its entry in the root
Cargo lockfile without committing them. The wheel metadata, `zplkit.__version__`, and
`zplkit.library_version` are checked for equality before upload.

Before the first automatic PyPI release, an account owner must configure a
[pending trusted publisher](https://docs.pypi.org/trusted-publishers/creating-a-project-through-oidc/)
at **PyPI → account → Publishing → Add a new pending publisher**:

| Field | Value |
| --- | --- |
| PyPI project name | `zplkit` |
| Owner | `codyps` |
| Repository name | `zpl` |
| Workflow filename | `release-plz.yml` |
| Environment name | Leave empty; this job does not declare an environment. |

The first successful trusted publication creates the project. A pending publisher
does not reserve the name. If the project already exists under your account, add
the same configuration under its **Publishing** settings instead. No initial
manual upload or long-lived PyPI API token is required. These settings must be
created on PyPI; repository configuration cannot create them. See
[PyPI's publisher setup](https://docs.pypi.org/trusted-publishers/adding-a-publisher/).

The workflow builds five stable-ABI wheels for CPython 3.10+:

- Linux x86_64 and aarch64, using Zig to target `manylinux_2_28` (glibc 2.28+).
- macOS x86_64 (Intel) and arm64 (Apple Silicon).
- Windows x86_64.

The reusable [distribution build workflow](../.github/workflows/python-package.yml)
also runs in ordinary PR CI. A source archive contains the linked local crates
for other systems with Rust installed. Each wheel is installed and tested on its
native runner; Linux also rebuilds and tests the source archive. All six files pass strict metadata checks.
Only after every build succeeds does a separate Ubuntu job receive
`id-token: write`, download the tested artifacts, and publish using the pinned
[PyPA action](https://github.com/pypa/gh-action-pypi-publish). The action generates
PyPI attestations by default. Build jobs have read-only repository permissions.

On retries, the workflow queries the PyPI release JSON API and omits only files
whose filename and SHA-256 match an existing, non-yanked file. It uploads the
remaining files after a partial failure. A changed file under an existing name,
yanked file, malformed registry response, timeout, or HTTP error other than 404
stops publication. Once all files match, the job succeeds without another upload.

If crates.io succeeds but PyPI fails, fix the publisher configuration and use
**Re-run failed jobs** on the original workflow run. The original tested artifacts
are retained for 14 days; retry within that window. Do not rebuild a published
version with changed bytes or advance its version just to bypass a failed upload.
A manual run on a later ordinary commit does not backfill an old release. Failed
Python builds or uploads stop release-PR preparation, alongside Rust/npm failures.

Verify the first publication on [PyPI](https://pypi.org/project/zplkit/) and
install it into a clean environment:

```sh
python -m pip install --only-binary=:all: zplkit==<released-version>
python -c 'import zplkit; print(zplkit.__version__, zplkit.library_version)'
```

Local release-gate tests, archive builds, and metadata checks do not establish
PyPI project ownership or a working OIDC exchange; the first successful Actions
publication verifies that setup.

## Hex publishing setup

The Elixir package is **`zpl`**, with Mix application `:zpl` and module `Zpl`.
The [Elixir distribution workflow](../.github/workflows/elixir-package.yml) runs
in PR CI and on completed stable renderer releases. It tests Elixir 1.15/OTP 25
and Elixir 1.18/OTP 28, then retains the tested source archive from the latter.
The archive includes this checkout's Rust runtime sources and a pruned copy of
the workspace lockfile. Consumers need Rust to compile the NIF; no prebuilt NIFs
are published. Generated HexDocs are not currently published.

The same shared release selector used by npm/PyPI requires the exact checked-out
commit's `zpl-v<version>` tag and a completed stable GitHub release. CI stamps
that version into the Mix project, binding Cargo manifest, and root lock entry.
The installed package version must match `Zpl.library_version()` before upload.
Elixir-only changes require an explicit renderer bump in the release PR, just
like Python/Node wrapper changes; development versions are not committed back.

Before the first automatic release:

1. Create/sign into the Hex account that will own `zpl`, enable two-factor
   authentication, and check that the public package name is available or owned
   by that account. A missing package lookup does not reserve its name.
2. In **Hex → Dashboard → API keys**, generate a publishing key with API write
   permission and an appropriate expiry. Add its value as the GitHub repository
   Actions secret **`HEX_API_KEY`** for `codyps/zpl`. Do not commit the key. The first
   automatic publication can create the package; afterwards prefer a key scoped
   to publishing `zpl`, and rotate/revoke the bootstrap key.
3. Merge this workflow before the next stable renderer release. Keep the secret
   available only to the publishing step; build/test jobs receive no Hex credential.

The workflow pins stable Hex **2.5.1** for archive creation. Hex's native GitHub
OIDC support is currently listed under **2.5.2-dev** in its
[changelog](https://github.com/hexpm/hex/blob/main/CHANGELOG.md); it is not enabled
here using an unreleased client. The stable
[CI publishing mechanism](https://hex.pm/docs/publish#publishing-from-ci) uses an
API key. This setup can move to native workload identity after a supported stable
release and account-side configuration.

The separate `elixir-release` job downloads the original tested artifact and
uploads those exact bytes through Hex's
[release API](https://github.com/hexpm/hex/blob/v2.5.1/src/mix_hex_api_release.erl).
It does not rebuild the tarball, publish documentation, or replace a release.
Before upload it validates the archive's package/application/version metadata
and queries Hex. Only HTTP 404 means the version is absent. An existing version
is skipped only when its outer SHA-256 checksum matches and it is not retired.
A changed checksum, retired release, malformed response, or network/auth error
fails visibly. POST requests are never automatically retried.

If crates.io succeeds and Hex fails, fix the account/secret and use **Re-run
failed jobs** on the original Actions run within the artifact's 14-day retention.
This preserves the original release commit and tested bytes. A manual workflow
run on a later ordinary commit does not backfill an earlier release. Hex failures
block new release-PR preparation alongside failures from the other registries.

Verify a first successful publication at
[hex.pm/packages/zpl](https://hex.pm/packages/zpl) and install the released version
in a clean Mix project with `{:zpl, "== <version>"}`. Confirm rendering and
`Zpl.library_version()`. Local tests and mocked uploads do not prove account
ownership or live publishing authorization; the first successful Actions upload
does. No live publication is needed to review this change.

## Releasing and verifying

Merge workflow changes into `main` before merging the generated release PR. Let
release-plz update the release PR and wait for its normal CI checks, then merge
it. Its push to `main` runs `release`, publishing any unpublished prepared
versions and creating their tags/releases. Merging only a workflow or feature PR
prepares a release PR; it does not itself publish a release.

Review `cargo package --list -p raster-diff -p zpl-bitmap-fonts -p zpl` before
publication. To verify the archives locally without uploading, run:

```sh
cargo package --locked -p raster-diff -p zpl-bitmap-fonts -p zpl
```

Packaging the crates together lets Cargo verify `zpl` against the prepared
`raster-diff` and `zpl-bitmap-fonts` archives before those versions are available
on crates.io. A
`release-plz release --dry-run` on an ordinary commit only tests the merge gate;
it does not prove packaging or OIDC authentication. A release-plz dry run on a
release commit publishes nothing, so its separate per-crate Cargo invocations
can fail to resolve an unpublished workspace dependency.

After merging a release PR, check the workflow's publishing job, each crate's
crates.io version, and the corresponding GitHub tag/release. A green
`release-pr` job alone is not evidence that publication succeeded. Trusted
publisher settings require crate-owner access, and actual OIDC exchange can only
be verified in GitHub Actions when publishing is needed.

If a release fails, inspect its logs and the registry/tag state before retrying
the failed workflow run. Manual dispatch on `main` still obeys the release-PR
gate, so it will skip publication if later ordinary commits have already landed.
Do not bypass the gate or change version numbers merely to retry publication.

References:

- [Release-plz quickstart and native trusted publishing](https://release-plz.dev/docs/github/quickstart)
- [Release-PR merge gate](https://release-plz.dev/docs/config#the-release_always-field)
- [crates.io trusted publishing](https://crates.io/docs/trusted-publishing)
- [Cargo package verification](https://doc.rust-lang.org/cargo/commands/cargo-package.html)
