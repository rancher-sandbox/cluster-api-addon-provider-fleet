{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  # Get dependencies from the main package
  inputsFrom = [ (pkgs.callPackage ./default.nix { }) ];
  # Additional tooling
  buildInputs = with pkgs; [
    rust-analyzer # LSP Server
    rustfmt       # Formatter
    clippy        # Linter
    just
    kind
    kubernetes-helm
    clusterctl
    kubectl
    k9s
    jq
    yq
    envsubst
    iproute2
    docker-client
    kustomize
  ];
}
