# sigma-updates

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
| `UPDATES_PUBLIC_BASE_URL` | Public base used in `bundle_url` and site links |
| `UPDATES_DEV_VERSION` | Override the built-in dev channel version |
| `SIGMA_INTERNAL_TOKEN` | Shared secret for publish/delete (same as other Sigma services) |
| `UPDATES_IDENTITY_PUBLIC_URL` | Identity BFF for header nav / CSP / Publish tab |
| `UPDATES_CONTACT_PUBLIC_URL` | Contact service (nav) |
| `UPDATES_CART_PUBLIC_URL` | Cart service (nav) |

## Publishing packages

Writes are authenticated the same way as catalog/store:

1. **Browser (OAuth)** — Sign in via Identity with realm role `sigma-admin`, open the **Publish** tab, upload a `.deb`. The page posts to Identity `/api/v1/packages` (session + CSRF); Identity requires admin and forwards with `x-sigma-internal-token`.
2. **CI / automation** — call updates directly with the shared secret:

```bash
curl -X POST "$SIGMA_UPDATES_URL/v1/packages" \
  -H "Authorization: Bearer $SIGMA_INTERNAL_TOKEN" \
  -H "X-Package-Filename: mypkg_1.0.0-1_all.deb" \
  --data-binary @mypkg_1.0.0-1_all.deb
```

`GET` list/download stays public for clients.

### Wingman hardware feed (i.MX 8M Plus)

**Policy:** publish **hardware** Yocto debs only (`MACHINE=sigma-racer-wingman-imx8mp`).
Never publish QEMU / `build-virt` / `sigma-racer-wingman-qemu` feeds — those are
local test images only.

#### Small sets (CLI)

```bash
export SIGMA_UPDATES_URL=http://updates.sigma.localtest.me:30080
export SIGMA_INTERNAL_TOKEN=dev-internal-token-32chars-minimum!!

# Example: M7 firmware only
cargo run -p sigma-updates-cli -- push \
  /path/to/sigma-racer-sidearm/dist/sigma-racer-sidearm-firmware_0.1.0-r0_all.deb \
  --allow-missing-deps

# Or a handful of product packages
cargo run -p sigma-updates-cli -- push \
  /path/to/build/tmp/deploy/deb/cortexa53-crypto-mx8mp/sigma-racer-cluster_git-r0_arm64.deb \
  /path/to/build/tmp/deploy/deb/cortexa53-crypto/sigma-racer-vehicle_1.0-r0_arm64.deb \
  --allow-missing-deps
```

`--allow-missing-deps` is normal for Yocto packages: rootfs deps (`libc6`,
`weston`, …) are not expected to live in the updates index.

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

## License

MIT OR Apache-2.0 for source code.
