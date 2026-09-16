# Browser preview / GitHub Pages

`site/` is a static, framework-free editor. `zpl-wasm/` exposes the existing
renderer through wasm-bindgen without adding browser dependencies to `zpl`.
The wrapper is an internal deployment package (`publish = false`).

ZPL stays in the browser: no rendering API, analytics, persistence, or printer
connection. PNG previews and PNG/SVG downloads use the native adapters. Multiple
labels are selectable. Edits invalidate old previews/downloads. Errors and font
fidelity warnings are displayed, not silently ignored.

Rendering runs in a fresh module worker for each request. Cancel, edits, and the
20-second timeout terminate the worker and release its Wasm memory. Input is
limited to 1 MiB and label dimensions to 4096 dots per side, including ZPL
overrides. These limits are resource safeguards, not a guarantee of printer parity.

## Local build

Install Rust, Node.js 22 or later, and the matching bindings CLI:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
cargo test -p zpl-wasm
bash scripts/build-site.sh
node --test site/tests/wasm.test.mjs
cd site
npm ci
npx playwright install chromium
npm test
cd ..
python3 -m http.server 8080 --directory _site
```

Open `http://localhost:8080/`. Serve over HTTP, not `file://`. All asset URLs are
relative, so the same output works under GitHub Pages' `/zpl/` project path.
Generated output in `_site/` is ignored by Git.

## Deployment

In repository **Settings → Pages → Build and deployment**, select **GitHub
Actions**. Merge the workflow into `main`; **Preview website** builds and tests
on pull requests, and deploys on pushes to `main` or manual runs on `main`.
The intended project URL is `https://codyps.github.io/zpl/` (unless a custom
domain is configured). The build job has read-only repository permissions;
only the deployment job has Pages and OIDC write permissions.

Only `_site/` is uploaded, never repository PDFs, capture campaigns, databases,
or other private files. The repository is currently private: Pages availability
depends on the account plan, and publishing the website does not make repository
source links publicly readable. No repository visibility change is made by this
workflow.

## References

- [wasm-bindgen deployment (web ES modules)](https://wasm-bindgen.github.io/wasm-bindgen/reference/deployment.html)
- [Module workers and worker termination](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API/Using_web_workers)
- [Blob URL lifetime](https://developer.mozilla.org/en-US/docs/Web/API/URL/revokeObjectURL_static)
- [GitHub Pages custom workflows](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages)
