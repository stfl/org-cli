# Repo-local pin of stfl/org-mcp plus the patched mcp-server-lib it depends
# on. Returns an attrset `{ org-mcp; mcp-server-lib; }` so a single callPackage
# invocation owns both shims:
#
#   ${org-mcp}/bin/org-mcp-stdio.sh
#   ${mcp-server-lib}/bin/emacs-mcp-stdio.sh
#
# Refresh the org-mcp pin with `nix/update-pins.sh org-mcp`.
{
  emacs,
  emacsPackagesFor,
  fetchFromGitHub,
  writeText,
}: let
  epkgs = emacsPackagesFor emacs;

  # The MELPA mcp-server-lib ships emacs-mcp-stdio.sh inside its elpa subdir
  # with a `#!/bin/bash` shebang (broken on NixOS without /bin/bash). Re-pack
  # so the script is exposed at $out/bin/ with a Nix-resolved bash shebang.
  mcp-server-lib = epkgs.mcp-server-lib.overrideAttrs (old: {
    postInstall =
      (old.postInstall or "")
      + ''
        script=$(find $out/share/emacs/site-lisp/elpa -name emacs-mcp-stdio.sh)
        install -Dm755 "$script" $out/bin/emacs-mcp-stdio.sh
        # Use --build (build-time PATH, which has stdenv's bash) — `--host`
        # requires bash to be in this package's runtime closure, which the
        # upstream MELPA derivation does not declare.
        patchShebangs --build $out/bin
      '';
  });

  rev = "3c2077b9d3758efb613926405fca3a6246343133";

  org-mcp = epkgs.melpaBuild {
    pname = "org-mcp";
    # MELPA-style YYYYMMDD.HHMM derived from the pinned commit's UTC committer
    # date by `nix/update-pins.sh`. Must match Emacs' canonical version form
    # (leading zeros stripped from HHMM) — otherwise the directory name baked
    # into the tar by package-build won't match what elpa2nix tries to untar.
    version = "20260429.848";
    commit = rev;
    src = fetchFromGitHub {
      owner = "stfl";
      repo = "org-mcp";
      inherit rev;
      hash = "sha256-pdIUMRN745tfU62xEfvtboYvBS7c5Y14q2sdpvURT60=";
    };
    # MELPA recipe — :files carries org-mcp-stdio.sh into
    # $out/share/emacs/site-lisp/elpa/org-mcp-<version>/, where upstream's
    # org-mcp--package-script-path picks it up via (locate-library "org-mcp").
    recipe = writeText "recipe" ''
      (org-mcp :fetcher github
               :repo "stfl/org-mcp"
               :files (:defaults "org-mcp-stdio.sh"))
    '';
    packageRequires = [epkgs.org epkgs.org-ql mcp-server-lib];
    # Expose org-mcp-stdio.sh on $out/bin so consumers can put the package on
    # PATH directly. Co-resolution with emacs-mcp-stdio.sh works via the
    # wrapper's PATH fallback (both packages on PATH).
    postInstall = ''
      script=$(find $out/share/emacs/site-lisp/elpa -name org-mcp-stdio.sh)
      install -Dm755 "$script" $out/bin/org-mcp-stdio.sh
      patchShebangs --build $out/bin
    '';
  };
in {
  inherit org-mcp mcp-server-lib;
}
