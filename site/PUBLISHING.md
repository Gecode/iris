# Publish the documentation site

The canonical URL is `https://iris.gecode.dev/`. Cloudflare's assets-only
Worker `iris-site` serves the contents of `dist/`. Astro emits directory
pages with trailing slashes, and Starlight generates `sitemap-index.xml` and
its sitemap shards from that canonical URL.

Production belongs to the **Gecode** Cloudflare account that owns
`gecode.dev`. The local Wrangler login for the **Zayenz** account cannot
access that zone. The configuration deliberately omits `account_id`;
CI selects the account through its secret.

## Set up production once

In the Iris repository's GitHub Actions settings, add these repository secrets:

- `CLOUDFLARE_API_TOKEN`: the existing `gecode-github-workers` token.
- `CLOUDFLARE_ACCOUNT_ID`: the Gecode account that owns `gecode.dev`.

The token has Workers Scripts Write, Workers Routes Write and Zone Read for
the intended account and zone. It has no DNS-write permission. Keep secret
values out of repository files and command output.

After the deployment changes land, a push to `main` that matches the
[site workflow](../.github/workflows/site.yml) builds, checks and deploys
`iris-site`. If either secret is absent, the build emits a notice and skips
the deploy job. After adding the secrets, rerun that push's workflow or make
another site change on `main`. Pull requests and manual workflow runs only
build and check.

After the first successful deployment creates the Worker, use the Cloudflare
dashboard in the **Gecode** account: **Workers → iris-site → Domains & Routes
→ Add custom domain**, then enter `iris.gecode.dev`. Cloudflare creates the
hostname's DNS record and certificate. Check that the hostname is unused
before attaching it. This connection is a Workers Custom Domain, rather than
a wildcard Worker route.

Alternatively, an operator logged into the Gecode account with DNS permission
can attach the domain through Wrangler. Select the Gecode account explicitly
with `CLOUDFLARE_ACCOUNT_ID`, temporarily add
`"routes": [{ "pattern": "iris.gecode.dev", "custom_domain": true }]` to
`wrangler.jsonc`, then deploy the checked build with `npx --no-install wrangler
deploy`. Remove that temporary `routes` entry afterwards, before committing.
Check the selected account with `npx --no-install wrangler whoami` first.

Only `iris-site` and its `iris.gecode.dev` Custom Domain belong to this
deployment. Leave every other Worker, route and DNS record in `gecode.dev`
unchanged. List Workers and Custom Domains before and after the one-time
setup, and retain the Worker version ID printed by Wrangler.

## Deploy a checked build

Use Node 22.12 or newer, npm, and `rsvg-convert`, as described in
[the site README](README.md). From this directory:

```sh
npm ci
npm run build
npm run check
npx --no-install wrangler deploy --dry-run
```

CI uploads this checked `dist/` as an artifact and deploys that same artifact
with the pinned Wrangler dependency. Its only GitHub permission is
`contents: read`. Runs for `main` are serialized, and an active deployment is
never cancelled by a newer push. GitHub may replace a pending run with the
newest push.

For an authorized manual deployment, select the Gecode account through
`CLOUDFLARE_ACCOUNT_ID`, confirm the login with `wrangler whoami`, then run:

```sh
npx --no-install wrangler deploy
```

The config has no `routes` entry and disables `workers.dev` and preview URLs.
Wrangler therefore uploads and activates a Worker version without reconciling
Custom Domains. An existing `iris.gecode.dev` connection stays attached.
Do not add the domain to the ordinary deployment config: Wrangler 4.156.1's
`publishCustomDomains` writes the domain records whenever a custom-domain
route is present, even if it already exists.

Cloudflare's [Workers permissions documentation](https://developers.cloudflare.com/workers/authorization/workers/)
says existing connections can serve new Worker versions without changing the
connection's permissions. The [Custom Domains documentation](https://developers.cloudflare.com/workers/configuration/routing/custom-domains/)
describes DNS creation during attachment. Omitting routes avoids domain and
DNS writes during subsequent CI deploys, so the existing token should suffice.
This conclusion follows the documentation and the pinned Wrangler's code;
deployment with that particular token remains unverified until the first
successful CI run. A local dry run does not test API permissions.

## Verify the result

For a local check of Cloudflare routing and headers, run:

```sh
npx --no-install wrangler dev --local --ip 127.0.0.1 --port 8788
```

Use `http://127.0.0.1:8788` for local requests. After publication, use the real
HTTPS origin without disabling certificate validation:

```sh
curl -sSI https://iris.gecode.dev/
curl -sSI https://iris.gecode.dev/introduction/
curl -sSI https://iris.gecode.dev/site-publishing-missing/
curl -sSI https://iris.gecode.dev/favicon.svg
curl -sS https://iris.gecode.dev/sitemap-index.xml
curl -sS https://iris.gecode.dev/sitemap-0.xml
```

Check response bodies as well as headers. The homepage and introduction must
contain their expected content and canonical URLs. A missing path must return
status 404 with the site's `404.html`, including for a nested missing path.
`/introduction` must redirect to `/introduction/`. The favicon must be SVG,
and sitemap entries must use `https://iris.gecode.dev/` with trailing slashes
for pages.

Choose a hashed `/_astro/` URL from the homepage and check it with `curl -sSI`.
[`public/_headers`](public/_headers) gives those assets
`Cache-Control: public, max-age=31536000, immutable`. Other assets, including
HTML, keep Cloudflare's default `public, max-age=0, must-revalidate` and ETag.
Search's Pagefind files also revalidate because their URLs are mutable.
See Cloudflare's [static asset headers](https://developers.cloudflare.com/workers/static-assets/headers/).

Record the source commit, Worker version ID, CI run URL and HTTPS checks with
each production deployment. TLS, DNS and token permissions require live checks;
local preview cannot verify them.

## Roll back

Pause automatic deployment while recovering so a queued push does not replace
the rollback. Record the current version and choose a retained, known-good
version from the correct account:

```sh
npx --no-install wrangler deployments list
npx --no-install wrangler versions list
npx --no-install wrangler rollback VERSION_ID
```

Rollback activates the selected version and its static assets; the Custom
Domain remains attached. The first deployment has no earlier version to
restore. See Cloudflare's [rollback documentation](https://developers.cloudflare.com/workers/configuration/versions-and-deployments/rollbacks/).

If the version is no longer retained, build a reviewed earlier commit in a
separate checkout, run its install, build and checks, and deploy its `dist/`
with this account-neutral Wrangler config. For a commit predating deployment
support, use the current pinned Wrangler and config with that earlier build's
assets. Restore the corresponding source on `main` through the normal review
process before re-enabling automatic deployment. Recheck pages, 404 handling,
headers, sitemap and TLS after either rollback path.
