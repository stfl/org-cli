# Repo-local pin of stfl/agile-gtd.el.
#
# Mirrors the packaging pattern used in ~/.config/dotfiles/packages/agile-gtd,
# but lives inside this repo so the live-test environment never reaches into
# user dotfiles. Refresh with `nix/update-pins.sh agile-gtd`.
{
  emacs,
  emacsPackagesFor,
  fetchFromGitHub,
}: let
  epkgs = emacsPackagesFor emacs;
in
  epkgs.trivialBuild {
    pname = "agile-gtd";
    version = "unstable-2026-04-15";
    src = fetchFromGitHub {
      owner = "stfl";
      repo = "agile-gtd.el";
      rev = "53f7d698117a12e258a839ece8fce5b3bcce0670";
      sha256 = "sha256-BD4GpknNhrZL6QSYnHMaHe+V0/3tJ5rBQUkFShubj9E=";
    };
    packageRequires = with epkgs; [org org-ql org-super-agenda org-edna dash s];
  }
