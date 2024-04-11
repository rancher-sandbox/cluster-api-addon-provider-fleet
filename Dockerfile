FROM cgr.dev/chainguard/static
COPY --chown=nonroot:nonroot ./_out/controller /app/
EXPOSE 8080
ENTRYPOINT ["/app/controller"]