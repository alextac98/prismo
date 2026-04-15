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
