#!/usr/bin/env bash
#
# Refresh GitHub-pinned Elisp packages used by the live-test environment.
#
# Usage:
#   nix/update-pins.sh                # update all
#   nix/update-pins.sh org-mcp        # update only org-mcp
#   nix/update-pins.sh agile-gtd      # update only agile-gtd
#
# Requires: curl, jq, nix (with flakes enabled). The script grabs the latest
# commit on the default branch from the GitHub API, rewrites rev/version, and
# lets `nix build` discover the new sha256 by tripping a fake-hash mismatch.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
FLAKE_ROOT="$(cd "${HERE}/.." && pwd)"
FAKE_HASH="sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="

need() { command -v "$1" >/dev/null || { echo "missing tool: $1" >&2; exit 1; }; }
need curl
need jq
need nix
need sed

update_pkg() {
  local pname="$1" owner="$2" repo="$3" branch="${4:-main}"
  local pkg_file="${HERE}/elisp/${pname}.nix"

  if [ ! -f "${pkg_file}" ]; then
    echo "[${pname}] package file not found: ${pkg_file}" >&2
    return 1
  fi

  echo "[${pname}] querying latest commit from ${owner}/${repo}@${branch}"
  local commit_json latest_rev latest_date current_rev
  commit_json=$(curl -fsSL "https://api.github.com/repos/${owner}/${repo}/commits/${branch}")
  latest_rev=$(echo "${commit_json}" | jq -r '.sha')
  latest_date=$(echo "${commit_json}" | jq -r '.commit.committer.date' | cut -c1-10)

  current_rev=$(grep 'rev = "' "${pkg_file}" | head -1 | sed 's/.*rev = "\(.*\)";.*/\1/')

  echo "  current: ${current_rev}"
  echo "  latest:  ${latest_rev}"

  if [ "${current_rev}" = "${latest_rev}" ]; then
    echo "  already up to date"
    return 0
  fi

  sed -i "s|rev = \"${current_rev}\";|rev = \"${latest_rev}\";|" "${pkg_file}"
  sed -i "s|version = \"unstable-[0-9-]*\";|version = \"unstable-${latest_date}\";|" "${pkg_file}"
  sed -i "s|sha256 = \"sha256-[^\"]*\";|sha256 = \"${FAKE_HASH}\";|" "${pkg_file}"

  echo "[${pname}] computing src hash"
  local build_out src_sri
  build_out=$(cd "${FLAKE_ROOT}" && nix build --no-link '.#live-test-env' 2>&1 || true)
  src_sri=$(echo "${build_out}" | awk '
    /got:[[:space:]]+sha256-/ { for (i=1;i<=NF;i++) if ($i ~ /^sha256-/) { print $i; exit } }
  ')

  if [ -z "${src_sri}" ]; then
    echo "!! could not determine hash; build output follows:" >&2
    echo "${build_out}" | tail -40 >&2
    return 1
  fi
  echo "  src hash: ${src_sri}"
  sed -i "s|${FAKE_HASH}|${src_sri}|" "${pkg_file}"

  echo "[${pname}] updated to ${latest_rev} (${latest_date})"
}

main() {
  local target="${1:-all}"
  case "${target}" in
    all)
      update_pkg org-mcp   stfl org-mcp
      update_pkg agile-gtd stfl agile-gtd.el
      ;;
    org-mcp)
      update_pkg org-mcp   stfl org-mcp
      ;;
    agile-gtd)
      update_pkg agile-gtd stfl agile-gtd.el
      ;;
    *)
      echo "unknown target: ${target}" >&2
      echo "usage: $0 [all|org-mcp|agile-gtd]" >&2
      exit 2
      ;;
  esac

  echo
  echo "verify: nix build .#live-test-env"
}

main "$@"
