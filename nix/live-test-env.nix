# Repo-local Emacs environment for org-cli live integration tests.
#
# Produces a single derivation with stable paths the Rust test harness (and
# CI) can consume without depending on user dotfiles, system Emacs, or a
# pre-running daemon:
#
#   $out/bin/emacs                  — wrapped Emacs (org + org-mcp + org-ql)
#   $out/bin/emacsclient            — matching client
#   $out/bin/emacs-mcp-stdio.sh     — patched launcher (stable shebang)
#   $out/share/org-cli-live/init.el — repo-local init.el
#
# The base env is intentionally minimal — just enough for org-mcp to register
# its tools. Tests that need extra Elisp (e.g. agile-gtd for GTD query
# bindings) load an overlay via `emacs -l <overlay.el>` after init.el.
#
# Plus passthru attrs for callers that prefer composing with Nix directly:
#   .emacs           — the wrapped emacs derivation
#   .mcpStdioShim    — the patched launcher script (single file)
#   .initEl          — path to the init.el source
{pkgs}: let
  emacs = pkgs.emacs;
  epkgs = pkgs.emacsPackagesFor emacs;

  org-mcp-pkg = pkgs.callPackage ./elisp/org-mcp.nix {
    inherit emacs;
    inherit (pkgs) emacsPackagesFor fetchFromGitHub;
  };

  # Minimal package set: just what org-mcp needs to register its tools.
  # Per-test fixtures that need GTD semantics layer agile-gtd via an overlay
  # file passed with `emacs -l <overlay.el>` — keeping it out of the base env.
  emacsWithOrgMcp = epkgs.emacsWithPackages (e:
    with e; [
      org
      org-ql
      mcp-server-lib
      org-mcp-pkg
    ]);

  # The stdio shim ships inside mcp-server-lib under a version-suffixed
  # directory. Extract it to a stable path and patch the `#!/bin/bash`
  # shebang via patchShebangs so it runs on hosts without /bin/bash (NixOS).
  mcpStdioShim =
    pkgs.runCommand "emacs-mcp-stdio.sh" {
      nativeBuildInputs = [pkgs.bash];
    } ''
      shim=$(find ${epkgs.mcp-server-lib}/share/emacs -name emacs-mcp-stdio.sh | head -1)
      if [ -z "$shim" ]; then
        echo "emacs-mcp-stdio.sh not found in mcp-server-lib" >&2
        exit 1
      fi
      install -m755 "$shim" $out
      patchShebangs $out
    '';
in
  pkgs.runCommand "org-cli-live-test-env" {
    passthru = {
      emacs = emacsWithOrgMcp;
      inherit mcpStdioShim;
      initEl = ./init.el;
    };
    meta = with pkgs.lib; {
      description = "Pinned Emacs + org-mcp + agile-gtd for org-cli live tests";
      platforms = platforms.unix;
    };
  } ''
    mkdir -p $out/bin $out/share/org-cli-live

    for prog in emacs emacsclient; do
      if [ -e ${emacsWithOrgMcp}/bin/$prog ]; then
        ln -s ${emacsWithOrgMcp}/bin/$prog $out/bin/$prog
      fi
    done

    install -m755 ${mcpStdioShim} $out/bin/emacs-mcp-stdio.sh
    install -m644 ${./init.el}    $out/share/org-cli-live/init.el
  ''
