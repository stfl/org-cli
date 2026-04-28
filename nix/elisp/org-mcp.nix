# Repo-local pin of stfl/org-mcp.
#
# Mirrors the packaging pattern used in ~/.config/dotfiles/packages/org-mcp,
# but lives inside this repo so the live-test environment never reaches into
# user dotfiles. Refresh with `nix/update-pins.sh org-mcp`.
{
  emacs,
  emacsPackagesFor,
  fetchFromGitHub,
}: let
  epkgs = emacsPackagesFor emacs;
in
  epkgs.trivialBuild {
    pname = "org-mcp";
    version = "unstable-2026-04-26";
    src = fetchFromGitHub {
      owner = "stfl";
      repo = "org-mcp";
      rev = "326c3aea76c8c77e8f87376e277e0c873b52b7ce";
      sha256 = "sha256-avMWY9RPBB5TBVppGDX8WFcFrcG1cFXKr7OubVeFsuA=";
    };
    packageRequires = with epkgs; [org org-ql mcp-server-lib];
  }
