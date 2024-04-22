FROM cgr.dev/chainguard/static
LABEL org.opencontainers.image.source=https://github.com/rancher-sandbox/cluster-api-addon-provider-fleet
COPY --chown=nonroot:nonroot ./_out/controller /app/
EXPOSE 8080
ENTRYPOINT ["/app/controller"]