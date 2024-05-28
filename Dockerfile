FROM cgr.dev/chainguard/static as build
LABEL org.opencontainers.image.source=https://github.com/rancher-sandbox/cluster-api-addon-provider-fleet
COPY --chown=nonroot:nonroot ./_out/controller /app/

FROM alpine/helm:3.14.4
COPY --from=build --chown=nonroot:nonroot /app/controller /apps/
ENV PATH="${PATH}:/apps"
EXPOSE 8080
ENTRYPOINT ["/apps/controller"]
