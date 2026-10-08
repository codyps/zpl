# Releases

The [Release-plz workflow](../.github/workflows/release-plz.yml) runs on pushes to
`main`, or manually from the Actions tab on `main`. It has three jobs:

1. `release` publishes prepared versions to **crates.io**, pushes package tags,
   and creates GitHub releases with changelog notes. With
   `release_always = false`, release-plz does this only when the checked-out
   commit is associated with a PR whose branch starts with `release-plz-`.
   Ordinary feature and maintenance merges skip publication.
2. `npm-release` publishes `@codyps/zpl` after a completed stable `zpl` release
   at the same commit. It builds and tests the Wasm package and uses the Rust
   renderer version as the npm version. Ordinary commits and releases of only
   other crates skip npm publication.
3. After both publishing jobs succeed, `release-pr` creates or updates a PR containing
   package versions, changelogs, and workspace dependency updates, using the
   published crates.io versions as the baseline. A failed publication stops
   release preparation so it cannot race ahead of the registry.

`raster-diff`, `zpl-bitmap-fonts`, and `zpl` are publishable and managed by
release-plz. The remaining workspace packages set `publish = false`. Release-plz
derives dependency order and publishes the raster and bitmap-font dependencies
before `zpl`. Tags and GitHub releases use `<crate>-v<version>`.

The action and Rust/checkout actions are pinned to commit SHAs; the release-plz
binary is pinned separately. Publishing jobs are serialized, and an active publish
is not cancelled by newer pushes; release-PR jobs are also serialized. Both jobs
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
