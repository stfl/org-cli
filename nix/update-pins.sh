#!/usr/bin/env bash
#
# Refresh GitHub-pinned Elisp packages used by the live-test environment.
#
# Usage:
#   nix/update-pins.sh                # update all
#   nix/update-pins.sh org-mcp        # update only org-mcp
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
  local commit_json latest_rev commit_iso latest_date latest_version current_rev
  commit_json=$(curl -fsSL "https://api.github.com/repos/${owner}/${repo}/commits/${branch}")
  latest_rev=$(echo "${commit_json}" | jq -r '.sha')
  # committer.date is ISO 8601 in UTC: YYYY-MM-DDTHH:MM:SSZ
  commit_iso=$(echo "${commit_json}" | jq -r '.commit.committer.date')
  latest_date=${commit_iso:0:10}
  # MELPA-style version: YYYYMMDD.HHMM (UTC). The HHMM tail is canonicalized
  # the same way Emacs' version-to-list / package-version-join does — leading
  # zeros stripped — so the Nix `version` matches the directory name baked
  # into the tar by package-build, which elpa2nix expects to untar cleanly.
  local ymd hhmm_raw hhmm_canon
  ymd="${commit_iso:0:4}${commit_iso:5:2}${commit_iso:8:2}"
  hhmm_raw="${commit_iso:11:2}${commit_iso:14:2}"
  hhmm_canon="${hhmm_raw#"${hhmm_raw%%[!0]*}"}"  # strip leading zeros
  hhmm_canon=${hhmm_canon:-0}
  latest_version="${ymd}.${hhmm_canon}"

  current_rev=$(grep 'rev = "' "${pkg_file}" | head -1 | sed 's/.*rev = "\(.*\)";.*/\1/')

  echo "  current: ${current_rev}"
  echo "  latest:  ${latest_rev}"
  echo "  version: ${latest_version} (${latest_date})"

  if [ "${current_rev}" = "${latest_rev}" ]; then
    echo "  already up to date"
    return 0
  fi

  sed -i "s|rev = \"${current_rev}\";|rev = \"${latest_rev}\";|" "${pkg_file}"
  # Match MELPA-style YYYYMMDD.N versions used by melpaBuild.
  sed -i -E "s|version = \"[0-9]+\.[0-9]+\";|version = \"${latest_version}\";|" "${pkg_file}"
  # melpaBuild uses fetchFromGitHub with `hash =` (SRI form).
  sed -i "s|hash = \"sha256-[^\"]*\";|hash = \"${FAKE_HASH}\";|" "${pkg_file}"

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

  echo "[${pname}] updated to ${latest_rev} version=${latest_version} (${latest_date})"
}

main() {
  local target="${1:-all}"
  case "${target}" in
    all|org-mcp)
      update_pkg org-mcp stfl org-mcp
      ;;
    *)
      echo "unknown target: ${target}" >&2
      echo "usage: $0 [all|org-mcp]" >&2
      exit 2
      ;;
  esac

  echo
  echo "verify: nix build .#live-test-env"
}

main "$@"
