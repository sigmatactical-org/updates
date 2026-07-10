#!/usr/bin/env bash
# Publish a Yocto *hardware* deploy/deb tree into the kind updates PVC.
#
# NEVER use this for QEMU / build-virt / sigma-racer-wingman-qemu feeds.
#
# Usage:
#   ./scripts/publish-yocto-feed.sh /path/to/build/tmp/deploy/deb
#
# Env:
#   KIND_NAMESPACE   (default: sigma-dev)
#   UPDATES_DEPLOY   (default: updates)
#   UPDATES_PVC      (default: updates-packages)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPLOY="${1:-}"
NS="${KIND_NAMESPACE:-sigma-dev}"
DEPLOYMENT="${UPDATES_DEPLOY:-updates}"
PVC="${UPDATES_PVC:-updates-packages}"
LOADER="updates-feed-loader"

if [[ -z "$DEPLOY" || ! -d "$DEPLOY" ]]; then
  echo "usage: $0 /path/to/yocto/tmp/deploy/deb" >&2
  exit 1
fi

# Refuse obvious QEMU / virt trees
case "$DEPLOY" in
  *build-virt*|*wingman-qemu*|*sigma_racer_wingman_qemu*)
    echo "error: refusing QEMU/virt deploy path: $DEPLOY" >&2
    echo "updates is for hardware (e.g. i.MX 8M Plus) feeds only." >&2
    exit 1
    ;;
esac

COUNT=$(find "$DEPLOY" -name '*.deb' | wc -l)
SIZE=$(du -sh "$DEPLOY" | awk '{print $1}')
echo "==> publishing $COUNT .deb ($SIZE) from $DEPLOY into $NS/$PVC"

echo "==> scale $DEPLOYMENT to 0 (RWO PVC)"
kubectl -n "$NS" scale "deploy/$DEPLOYMENT" --replicas=0
kubectl -n "$NS" wait --for=delete pod -l "app=$DEPLOYMENT" --timeout=180s 2>/dev/null || true

echo "==> start loader pod"
kubectl -n "$NS" delete pod "$LOADER" --ignore-not-found --force --grace-period=0 2>/dev/null || true
cat <<EOF | kubectl -n "$NS" apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: $LOADER
spec:
  restartPolicy: Never
  containers:
    - name: loader
      image: busybox:1.36
      command: ["sh", "-c", "sleep 7200"]
      volumeMounts:
        - name: packages
          mountPath: /packages
  volumes:
    - name: packages
      persistentVolumeClaim:
        claimName: $PVC
EOF
kubectl -n "$NS" wait --for=condition=Ready "pod/$LOADER" --timeout=120s

echo "==> wipe existing packages on PVC"
kubectl -n "$NS" exec "$LOADER" -- sh -c 'rm -rf /packages/* /packages/.[!.]* 2>/dev/null; mkdir -p /packages'

echo "==> stream flattened .deb tree into PVC"
# Flatten nested Yocto arch dirs (all/, cortexa53-crypto/, machine/, …)
tar -C "$DEPLOY" --transform 's|.*/||' -cf - \
  $(find "$DEPLOY" -name '*.deb' -printf '%P\n') \
  | kubectl -n "$NS" exec -i "$LOADER" -- tar -C /packages -xf -

echo "==> ownership for distroless nonroot (65532)"
kubectl -n "$NS" exec "$LOADER" -- sh -c \
  'chown -R 65532:65532 /packages; find /packages -name "*.deb" | wc -l; du -sh /packages'

echo "==> tear down loader, scale $DEPLOYMENT back"
kubectl -n "$NS" delete pod "$LOADER" --force --grace-period=0
kubectl -n "$NS" scale "deploy/$DEPLOYMENT" --replicas=1
kubectl -n "$NS" rollout status "deploy/$DEPLOYMENT" --timeout=180s

echo "==> done. Verify with:"
echo "  curl -sS \"\${SIGMA_UPDATES_URL:-http://updates.sigma.localtest.me:30080}/v1/packages?page=1&per_page=5\""
