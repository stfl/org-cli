# Repo-local Emacs environment for org-cli live integration tests.
#
# Produces a single derivation with stable paths the Rust test harness (and
# CI) can consume without depending on user dotfiles, system Emacs, or a
# pre-running daemon:
#
#   $out/bin/emacs                  — wrapped Emacs (org + org-mcp + org-ql)
#   $out/bin/emacsclient            — matching client
#   $out/share/org-cli-live/init.el — repo-local init.el
#   $out/share/org-cli-live/paths.env — Nix-baked paths to both stdio shims
#                                     and their bin/ dirs (for PATH-based
#                                     co-resolution); source it in shell
#                                     scripts (no `find`).
#
# The base env is intentionally minimal — just enough for org-mcp to register
# its tools. Tests that need extra Elisp (e.g. agile-gtd for GTD query
# bindings) load an overlay via `emacs -l <overlay.el>` after init.el.
#
# Plus passthru attrs for callers that prefer composing with Nix directly:
#   .emacs            — the wrapped emacs derivation
#   .orgMcpPkg        — the pinned org-mcp package (ships $out/bin shim)
#   .mcpServerLibPkg  — the patched mcp-server-lib package (ships $out/bin shim)
#   .orgMcpStdio      — absolute path to org-mcp-stdio.sh
#   .emacsMcpStdio    — absolute path to emacs-mcp-stdio.sh
#   .initEl           — path to the init.el source
{pkgs}: let
  emacs = pkgs.emacs;
  epkgs = pkgs.emacsPackagesFor emacs;

  # `org-mcp.nix` returns an attrset that owns both packages so emacs-mcp-lib
  # is patched in lockstep with the org-mcp dep that imports it.
  orgMcp = pkgs.callPackage ./elisp/org-mcp.nix {
    inherit emacs;
    inherit (pkgs) emacsPackagesFor fetchFromGitHub writeText;
  };
  inherit (orgMcp) org-mcp mcp-server-lib;

  # Minimal package set: just what org-mcp needs to register its tools.
  # Per-test fixtures that need GTD semantics layer agile-gtd via an overlay
  # file passed with `emacs -l <overlay.el>` — keeping it out of the base env.
  emacsWithOrgMcp = epkgs.emacsWithPackages (e: [
    e.org
    e.org-ql
    mcp-server-lib
    org-mcp
  ]);

  orgMcpStdioPath = "${org-mcp}/bin/org-mcp-stdio.sh";
  emacsMcpStdioPath = "${mcp-server-lib}/bin/emacs-mcp-stdio.sh";
in
  pkgs.runCommand "org-cli-live-test-env" {
    passthru = {
      emacs = emacsWithOrgMcp;
      orgMcpPkg = org-mcp;
      mcpServerLibPkg = mcp-server-lib;
      orgMcpStdio = orgMcpStdioPath;
      emacsMcpStdio = emacsMcpStdioPath;
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

    install -m644 ${./init.el} $out/share/org-cli-live/init.el

    # Bake deterministic paths to both shims and their bin/ directories. The
    # org-mcp wrapper's PATH fallback resolves emacs-mcp-stdio.sh, so callers
    # only need to put both bin/ dirs on PATH.
    cat > $out/share/org-cli-live/paths.env <<EOF
    ORG_MCP_STDIO=${orgMcpStdioPath}
    EMACS_MCP_STDIO=${emacsMcpStdioPath}
    ORG_MCP_BIN=${org-mcp}/bin
    EMACS_MCP_LIB_BIN=${mcp-server-lib}/bin
    EOF
  ''
