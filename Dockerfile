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
COPY --chown=nonroot:nonroot dbc /app/dbc
COPY --chown=nonroot:nonroot vss /app/vss
USER nonroot:nonroot
ENV PORT=8080
ENV UPDATES_PACKAGES_DIR=/app/packages
ENV UPDATES_DBC_DIR=/app/dbc
ENV UPDATES_VSS_DIR=/app/vss
EXPOSE 8080
ENTRYPOINT ["/app/sigma-updates"]
