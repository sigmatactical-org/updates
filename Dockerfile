# syntax=docker/dockerfile:1.6
FROM debian:bookworm-slim AS runtime-libs
RUN apt-get update \
    && apt-get install -y --no-install-recommends liblzma5 \
    && rm -rf /var/lib/apt/lists/*

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:d3cda6e91129130d7229a1806b6a73d292ef245ab032da7851907798024cefba

WORKDIR /app
# xz2 links liblzma; distroless/cc does not ship it.
COPY --from=runtime-libs /usr/lib/x86_64-linux-gnu/liblzma.so.5 /usr/lib/x86_64-linux-gnu/liblzma.so.5
COPY --chmod=555 sigma-updates /app/sigma-updates
COPY --chown=nonroot:nonroot packages /app/packages
USER nonroot:nonroot
ENV PORT=8080
ENV UPDATES_PACKAGES_DIR=/app/packages
EXPOSE 8080
ENTRYPOINT ["/app/sigma-updates"]
