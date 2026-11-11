#!/usr/bin/env bash
# Self-test of the delivered leak guard (scripts/smokestack/hooks/leak-guard.sh), run by
# scripts/qc.ps1 on every gate (audit-secret-handling-retention-1794018053). repose owns this
# file. Since smokestack 0.7.2, smokestack's own self-test is delivered here too, as
# scripts/smokestack/check-leak-guard.sh; moving qc.ps1 to it and retiring this copy is later
# work (Roger #40985, Morticia #40988). Change the guard in smokestack, not here.
#
# The guard runs in a scratch git repository under the temp folder, against content staged there.
# Every leaky fixture is assembled at run time, so this file never holds a string the guard would
# flag, and LEAK_GUARD_PATTERNS always names a fixture file here: the real pattern file is never
# read. No matched value is printed. The scratch folder is the only thing removed.
set -euo pipefail

self_dir="$(cd "$(dirname "$0")" && pwd)"
hooks="$self_dir/smokestack/hooks"
[ -f "$hooks/leak-guard.sh" ] || { echo "check-leak-guard: no $hooks/leak-guard.sh" >&2; exit 2; }

# From a hook or a pathspec commit, git's environment points at the real repository; the scratch
# repository must not write into it.
# shellcheck disable=SC2046 # one variable name per word, which unset needs split
unset $(git rev-parse --local-env-vars)

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
repo="$tmp/repo"
guard=scripts/smokestack/hooks/leak-guard.sh
mkdir -p "$repo/$(dirname "$guard")"
git -C "$repo" init -q
git -C "$repo" config core.autocrlf false
# A hooksPath set globally would run its hooks on the scratch commit; only the copied guard, run
# by hand below, may run here (Roger Low on 44c64a0).
mkdir "$tmp/no-hooks"
git -C "$repo" config core.hooksPath "$tmp/no-hooks"
git -C "$repo" -c user.email=t@test.invalid -c user.name=t commit -q --allow-empty -m init
# The guard sources its decoder from its own folder when a binary file is staged.
cp "$hooks/leak-guard.sh" "$hooks/leak-guard-decode.sh" "$repo/$(dirname "$guard")/"

patterns="$tmp/patterns"
printf '%s\n' '# fixture private patterns for the self-test' '' '\bzzq-fixture-literal\b' >"$patterns"

failures=0
fail() {
	echo "check-leak-guard: FAIL - $1" >&2
	failures=$((failures + 1))
}

# Stages $2 and runs the guard with pattern file $1; prints its exit code. The guard's own output
# lands in $tmp/out, which is checked for the staged value where a case says so.
run_guard() {
	printf '%s\n' "$2" >"$repo/staged.txt"
	git -C "$repo" add staged.txt
	local code=0
	(cd "$repo" && LEAK_GUARD_PATTERNS="$1" bash "$guard") >"$tmp/out" 2>&1 || code=$?
	echo "$code"
}

# 1. A clean staging passes.
[ "$(run_guard "$patterns" 'plain harmless content')" = 0 ] || fail 'clean content was refused'

# 2. A GitHub token shape at real length (ghp_ and 36 more) is refused, and not echoed back.
token="ghp_$(printf 'a%.0s' $(seq 1 36))"
[ "$(run_guard "$patterns" "token=$token")" = 1 ] || fail 'a ghp_ token shape was not refused'
grep -qF "$token" "$tmp/out" && fail 'the refusal printed the staged token'

# 3. A PEM header with a base64 key body is refused.
pem="-----BEGIN RSA $(printf 'PRIVATE') KEY-----"
body="$(printf 'A%.0s' $(seq 1 64))"
[ "$(run_guard "$patterns" "$(printf '%s\n%s' "$pem" "$body")")" = 1 ] || fail 'a PEM key block was not refused'

# 4. A private literal from the configured pattern file is refused, and not echoed back.
literal="zzq-fixture-$(printf 'literal')"
[ "$(run_guard "$patterns" "mentions $literal here")" = 1 ] || fail 'a tier-2 literal was not refused'
grep -qF "$literal" "$tmp/out" && fail 'the refusal printed the staged literal'

# 5. A configured pattern file that is missing refuses even clean content (fail closed).
[ "$(run_guard "$tmp/no-such-patterns" 'plain harmless content')" = 1 ] || fail 'a missing pattern file let a commit through'

if [ "$failures" -gt 0 ]; then
	echo "check-leak-guard: $failures case(s) failed" >&2
	exit 1
fi
echo 'check-leak-guard: 5 cases passed'
