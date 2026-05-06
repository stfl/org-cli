{
  description = "org — synchronous Rust CLI for the org-mcp Emacs MCP server";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    # Pin the toolchain used by `nix build` via fenix so CI / package builds
    # are reproducible across nixpkgs revisions. The devShell intentionally
    # does NOT install rust — developers bring their own (rustup, system).
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix }:
    let
      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

      # Build org-cli against an arbitrary `pkgs`. Used by both
      # `packages.default` (per-system) and `overlays.default` (consumer
      # nixpkgs), so downstream flakes can pull `pkgs.org-cli` after
      # applying the overlay.
      mkOrgCli = pkgs:
        let
          # Use minimal toolchain (cargo + rustc + rust-std only). The full
          # `stable.toolchain` pulls preview components (rust-analyzer-preview,
          # llvm-bitcode-linker-preview, llvm-tools-preview) that aren't needed
          # for `buildRustPackage` and have caused empty-hash failures in CI.
          toolchain = fenix.packages.${pkgs.system}.minimal.toolchain;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
        in
        rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          version = cargoToml.package.version;

          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              let base = baseNameOf path; in
              !(builtins.elem base [ ".beads" ".omc" "target" "PLAN.md" ".direnv" ])
              && !(pkgs.lib.hasSuffix ".sqlite3" base);
          };

          cargoLock.lockFile = ./Cargo.lock;

          # Tests spawn the mock_org_mcp binary built from the same crate.
          # The live integration test is gated on ORG_LIVE_TEST=1 (no-op in sandbox).
          doCheck = true;

          # mock_org_mcp is a test fixture, not part of the shipped CLI.
          postInstall = ''
            rm -f $out/bin/mock_org_mcp
          '';

          meta = with pkgs.lib; {
            description = "Synchronous Rust CLI for the org-mcp Emacs MCP server";
            longDescription = ''
              Agent-first CLI exposing org-mcp tools as deterministic JSON
              subcommands. Hand-rolled sync MCP client over line-delimited
              JSON-RPC stdio. JSON envelope on stdout, structured stderr,
              meaningful exit codes. See PLAN.md for the v1 contract.
            '';
            mainProgram = "org";
            platforms = platforms.unix;
          };
        };
    in
    {
      overlays.default = _final: prev: {
        org-cli = mkOrgCli prev;
      };
    }
    // flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        packages.default = mkOrgCli pkgs;

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/org";
        };

        # Repo-local Emacs + org-mcp + agile-gtd environment for live
        # integration tests. See nix/live-test-env.nix for the contract:
        #   $out/bin/emacs, bin/emacsclient, bin/emacs-mcp-stdio.sh,
        #   share/org-cli-live/init.el.
        # Built independently of `packages.default` so normal `nix build`
        # never pulls in Emacs.
        packages.live-test-env = pkgs.callPackage ./nix/live-test-env.nix { inherit pkgs; };

        # Dev shell intentionally has NO rust toolchain — bring your own
        # (rustup, system rust, direnv layout). Only project-specific tooling
        # like `just` lives here.
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            just
          ];
        };

        checks.default = self.packages.${system}.default;
        checks.live-test-env = self.packages.${system}.live-test-env;
      });
}
