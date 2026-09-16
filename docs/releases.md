# Release pull requests

The [release-plz workflow](../.github/workflows/release-plz.yml) runs on pushes to
`main`, or manually from the Actions tab on `main`. It creates or updates a PR
containing package versions, changelogs, and workspace dependency updates, using
published **crates.io** versions as the baseline.

It runs **only `release-pr`**: merging its PR does not publish crates, push release
tags, or create a GitHub release. No crates.io token is needed for this workflow.
The action and Rust/checkout actions are pinned to commit SHAs; the release-plz
binary is pinned separately. Concurrent release-PR jobs are serialized.

## Repository setup

1. Merge the workflow and configuration into `main`.
2. In **Settings → Actions → General → Workflow permissions**, enable
   **Allow GitHub Actions to create and approve pull requests**. This was disabled
   when the workflow was added. The workflow itself grants only the required
   `contents: write` and `pull-requests: write` permissions; no auto-approval step
   is configured.
3. Run **Release PR** manually or push a commit to `main`.

The default `GITHUB_TOKEN` is sufficient to create the PR. GitHub does not trigger
ordinary PR CI from PRs created with that token. If release PRs must trigger CI,
use a dedicated GitHub App token or appropriately scoped PAT for the release-plz
step, following the [token documentation](https://release-plz.dev/docs/github/token).
Do not put tokens in repository files.

## Publishing later

Publishing is intentionally a separate step. Before enabling it, confirm crate
name ownership, complete package metadata,
and configure crates.io authentication. The repository is currently private;
publishing a crate makes its packaged source public. Review `cargo package
--list` for each package before publication.

`release_always = false` is configured so that a future release job can be gated
on merging a release PR. No publishing job or credentials are added here.

References:

- [Release-plz quickstart and PR-only setup](https://release-plz.dev/docs/github/quickstart)
- [Configuration fields](https://release-plz.dev/docs/config)
- [Action inputs](https://release-plz.dev/docs/github/input)
