# sigma-updates

[![CI](https://github.com/sigmatactical-org/updates/actions/workflows/ci.yml/badge.svg)](https://github.com/sigmatactical-org/updates/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![MSRV](https://img.shields.io/badge/MSRV-1.97.0-blue.svg)](https://www.rust-lang.org)

Debian **package index** and OTA **catalog** for Sigma. Serves `.deb` files from
this service’s own `packages/` directory, plus channel metadata for signed RAUC
bundles (Wingman).

Repository: https://github.com/sigmatactical-org/updates

## Web UI

Dev ingress: **`http://updates.sigma.localtest.me:30080/`**

- **Packages** — paginated `.deb` index with search (download links)
- **API** — endpoint reference for clients

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/health` | Liveness |
| `GET` | `/v1/packages` | JSON page (`?page=1&per_page=50&q=`; max 500/page) |
| `POST` | `/v1/packages` | Publish a `.deb` (`X-Package-Filename` + body; auth required) |
| `DELETE` | `/v1/packages/{file}.deb` | Remove a package (auth required) |
| `GET` | `/packages/{file}.deb` | Download a package |
| `GET` | `/v1/dbc` | JSON page of Sigma Racer `.dbc` schemas (`?page=1&per_page=50&q=`) |
| `GET` | `/v1/dbc/latest` | Latest Sigma Racer DBC metadata (prefers `sigma-racer.dbc`) |
| `POST` | `/v1/dbc` | Publish a `.dbc` (`X-Dbc-Filename` + body; auth required) |
| `DELETE` | `/v1/dbc/{file}.dbc` | Remove a schema file (auth required) |
| `GET` | `/dbc/{file}.dbc` | Download a DBC schema |
| `GET` | `/v1/channels` | List RAUC channels |
| `GET` | `/v1/channel/{name}/latest` | Latest RAUC release metadata |
| `GET` | `/v1/channel/{name}/bundle/{file}` | Bundle bytes (when published) |

## Client library & CLI

Workspace crates:

- `sigma-updates-deb` — parse `.deb` control / Depends / Provides
- `sigma-updates-client` — HTTP client + dependency-aware push planner
- `sigma-updates-cli` — command-line tool

```bash
cargo run -p sigma-updates-cli -- list \
  --url http://updates.sigma.localtest.me:30080

# Check deps against the remote index (no upload)
cargo run -p sigma-updates-cli -- check ./packages \
  --url http://updates.sigma.localtest.me:30080

# Publish in dependency order; refuses if Depends are missing
export SIGMA_INTERNAL_TOKEN=dev-internal-token-32chars-minimum!!
cargo run -p sigma-updates-cli -- push ./packages \
  --url http://updates.sigma.localtest.me:30080
```

`push` topo-sorts the local set, treats other locals as available for later packages,
and errors unless every `Depends` / `Pre-Depends` is satisfied by the remote index
(or by an earlier package in the same push). Use `--allow-missing-deps` only when you
intentionally publish incomplete sets.

## Configuration

| Variable | Purpose |
| --- | --- |
| `PORT` | Listen port (default `8080`) |
| `UPDATES_PACKAGES_DIR` | Directory of `.deb` files (default `packages`, image: `/app/packages`) |
| `UPDATES_DBC_DIR` | Directory of Sigma Racer `.dbc` schemas (default `dbc`, image: `/app/dbc`) |
| `UPDATES_PUBLIC_BASE_URL` | Public base used in `bundle_url` and site links |
| `UPDATES_DEV_VERSION` | Override the built-in dev channel version |
| `SIGMA_INTERNAL_TOKEN` | Shared secret for publish/delete (same as other Sigma services) |
| `UPDATES_IDENTITY_PUBLIC_URL` | Identity BFF for header nav / CSP / Publish tab |
| `UPDATES_CONTACT_PUBLIC_URL` | Contact service (nav) |
| `UPDATES_CART_PUBLIC_URL` | Cart service (nav) |

## Publishing packages

Writes go through one of:

1. **Browser (OAuth)** — Sign in via Identity with realm role `sigma-admin`, open the **Publish** tab, upload a `.deb`. The page posts to Identity `/api/v1/packages` (session + CSRF); Identity requires admin and forwards with `x-sigma-internal-token`.
2. **CI / OIDC client-credentials** — `sigma-updates-cli` obtains a Keycloak access token for client `sigma-updates-ci` (service account with `sigma-admin`) and posts to Identity `/api/v1/packages` (Bearer JWT; no CSRF). Identity forwards with `x-sigma-internal-token`.
3. **Direct shared secret (local/dev)** — call updates with `SIGMA_INTERNAL_TOKEN`:

```bash
curl -X POST "$SIGMA_UPDATES_URL/v1/packages" \
  -H "Authorization: Bearer $SIGMA_INTERNAL_TOKEN" \
  -H "X-Package-Filename: mypkg_1.0.0-1_all.deb" \
  --data-binary @mypkg_1.0.0-1_all.deb
```

`GET` list/download stays public for clients.

### CLI (OIDC → Identity)

```bash
export SIGMA_UPDATES_URL=https://identity.sigma.localtest.me:30443/api
export SIGMA_OIDC_CLIENT_ID=sigma-updates-ci
export SIGMA_OIDC_CLIENT_SECRET=dev-sigma-updates-ci-secret-change-me
export SIGMA_OIDC_ISSUER=https://keycloak.sigma.localtest.me:30443/realms/multcorp

cargo run -p sigma-updates-cli -- push ./packages --allow-missing-deps
```

Or pass flags: `--oidc-client-id`, `--oidc-client-secret`, `--oidc-token-url` / `--oidc-issuer`.

Direct updates (no Identity) still works with `--token` / `SIGMA_INTERNAL_TOKEN` and `--url` pointing at the updates service.

### Wingman hardware feed (i.MX 8M Plus)

**Policy:** publish **hardware** Yocto debs only (`MACHINE=sigma-racer-wingman-imx8mp`).
Never publish QEMU / `build-virt` / `sigma-racer-wingman-qemu` feeds — those are
local test images only.

#### Small sets (CLI)

```bash
# Via Identity (preferred for CI)
export SIGMA_UPDATES_URL=https://identity.sigma.localtest.me:30443/api
export SIGMA_OIDC_CLIENT_ID=sigma-updates-ci
export SIGMA_OIDC_CLIENT_SECRET=…
export SIGMA_OIDC_ISSUER=https://keycloak.sigma.localtest.me:30443/realms/multcorp

cargo run -p sigma-updates-cli -- push \
  /path/to/build/tmp/deploy/deb/cortexa53-crypto-mx8mp/sigma-racer-cluster_git-r0_arm64.deb \
  /path/to/build/tmp/deploy/deb/cortexa53-crypto/sigma-racer-vehicle_1.0-r0_arm64.deb \
  --allow-missing-deps

# Or direct to updates with shared secret (local)
export SIGMA_UPDATES_URL=http://updates.sigma.localtest.me:30080
export SIGMA_INTERNAL_TOKEN=dev-internal-token-32chars-minimum!!
cargo run -p sigma-updates-cli -- push ./packages --allow-missing-deps
```

`--allow-missing-deps` is normal for Yocto packages: rootfs deps (`libc6`,
`weston`, …) are not expected to live in the updates index.

Wingman release CI runs `embedded/sigma-racer-wingman/scripts/ci/publish-product-debs.sh`
after the imx8mp image build.

#### Full deploy tree (thousands of `.deb`)

HTTP push of an entire `tmp/deploy/deb` tree is too slow. Load the feed onto the
kind PVC instead (distroless has no shell/`tar`, so the script scales updates
down, uses a busybox loader pod, then scales back up):

```bash
# After: bitbake sigma-racer-wingman-image  (MACHINE=sigma-racer-wingman-imx8mp)
DEPLOY=$HOME/Source/sigma/embedded/sigma-racer-wingman/build/tmp/deploy/deb

./scripts/publish-yocto-feed.sh "$DEPLOY"

# Confirm
export SIGMA_UPDATES_URL=http://updates.sigma.localtest.me:30080
curl -sS "$SIGMA_UPDATES_URL/v1/packages?page=1&per_page=5" | jq '{total, page, sample: [.packages[].filename]}'
cargo run -p sigma-updates-cli -- list | head
```

The script **refuses** paths that look like QEMU/virt deploys and **replaces**
whatever is currently on the PVC.

Requires: `kubectl` context for the kind cluster, PVC `updates-packages`
(10 Gi in the platform overlay), and the updates deployment in `sigma-dev`.

## Local development

```bash
# drop .deb files into ./packages, or publish with sigma-updates-cli
UPDATES_PUBLIC_BASE_URL=http://127.0.0.1:8080 cargo run
# open http://127.0.0.1:8080/
```

## Docker

```bash
./scripts/docker-build.sh
docker build -f Dockerfile build/image -t sigma-updates:local
```

## Platform (kind)

Manifests: [platform](https://github.com/sigmatactical-org/platform) → `services/updates/`.

Cluster / testbed (RAUC catalog):

```bash
export SIGMA_UPDATES_URL=http://updates.sigma.localtest.me:30080
export SIGMA_UPDATES_CHANNEL=dev
export SIGMA_IMAGE_VERSION=0.0.0
```

## Brand & artwork

© Sigma Tactical Group. **All rights reserved.**

The Sigma Tactical Group name, logos, marks, artwork, and visual identity are **proprietary**. They are not covered by this repository's source-code license. See [BRANDING.md](BRANDING.md).

## License

MIT OR Apache-2.0 for source code.
