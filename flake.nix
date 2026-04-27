{
  description = "org — synchronous Rust CLI for the org-mcp Emacs MCP server";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      in
      {
        packages.default = pkgs.rustPlatform.buildRustPackage {
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

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/org";
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
          ];
        };

        checks.default = self.packages.${system}.default;
      });
}
