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
    fzf
    kind
    kubernetes-helm
    clusterctl
    kubectl
    k9s
    jq
    yq
    docker-client
    kustomize
  ];
}
