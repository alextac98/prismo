# Website

This site is built with [Docusaurus](https://docusaurus.io/) and uses the
top-level `docs/` directory as the source for the documentation section.

## Dependency Updates

The Bazel rules use `pnpm` as the package manager. To update
`website/pnpm-lock.yaml`, run:

```bash
cd website
bazel run -- @pnpm --dir $PWD install --lockfile-only
```

Build the static site with:

```bash
bazel build //website:site
```

Run the live-reloading development server with:

```bash
bazel run //website:dev
```

Preview the built site locally with:

```bash
bazel run //website:serve
```

The generated Pages artifact is written to:

```bash
bazel-bin/website/site
```

The site defaults to GitHub Pages-style values:

- `DOCS_SITE_URL=https://prismo.alextac.com`
- `DOCS_BASE_URL=/`

Override those environment variables in CI if the published URL changes.

## Vercel

This repository includes a root-level `vercel.json` so Vercel can build the
website through Bazel instead of running Docusaurus directly.

Use these project settings:

- Root Directory: repository root
- Framework Preset: Other
- Build Command: use `vercel.json`
- Output Directory: use `vercel.json`

The build command uses Bazelisk via `npx` and forwards these environment
variables into the Bazel action so Docusaurus can compute the published site
URL correctly:

- `VERCEL_URL`
- `DOCS_SITE_URL`
- `DOCS_BASE_URL`
- `DOCS_REPO_URL`

If you want the production site to publish at a custom domain, set
`DOCS_SITE_URL` in Vercel to the full origin, for example:

```bash
https://prismo.alextac.com
```
