FROM --platform=$BUILDPLATFORM registry.suse.com/bci/rust:1.84 AS build
LABEL org.opencontainers.image.source=https://github.com/rancher-sandbox/cluster-api-addon-provider-fleet
COPY --chown=nonroot:nonroot ./ /src/
WORKDIR /src
ARG features=""
RUN --mount=type=cache,target=/root/.cargo cargo build --features=${features} --release --bin controller

FROM registry.suse.com/suse/helm:3.13
COPY --from=build --chown=nonroot:nonroot /src/target/release/controller /apps/controller
ENV PATH="${PATH}:/apps"
EXPOSE 8080
ENTRYPOINT ["/apps/controller"]
