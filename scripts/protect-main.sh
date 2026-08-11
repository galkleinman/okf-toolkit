#!/usr/bin/env bash
# Applies the branch rules for `main`.
#
# Two rulesets are created:
#
#   okf-toolkit: main   pull requests only, no direct pushes, CI must pass
#   okf-toolkit: branches  only the maintainer may create branches in this
#                          repository, so contributors work from forks
#
# Rulesets need a public repository or a paid plan; on a private repository
# with a free personal account the API returns 403.
#
# Re-running is safe: an existing ruleset with the same name is updated.
set -euo pipefail

REPO="${1:-galkleinman/okf-toolkit}"
OWNER="${REPO%%/*}"

require_rulesets() {
    if ! gh api "repos/${REPO}/rulesets" >/dev/null 2>&1; then
        cat >&2 <<EOF
error: cannot read rulesets for ${REPO}.

Rulesets require a public repository or a paid plan. Make the repository
public, or upgrade the account, then run this script again.
EOF
        exit 1
    fi
}

# Finds an existing ruleset id by name, so the script updates instead of
# creating a duplicate on a second run.
ruleset_id() {
    gh api "repos/${REPO}/rulesets" --jq ".[] | select(.name == \"$1\") | .id" | head -n 1
}

apply() {
    local name="$1" payload="$2" id
    id="$(ruleset_id "${name}")"
    if [ -n "${id}" ]; then
        echo "updating ruleset '${name}' (${id})"
        gh api -X PUT "repos/${REPO}/rulesets/${id}" --input - <<<"${payload}" >/dev/null
    else
        echo "creating ruleset '${name}'"
        gh api -X POST "repos/${REPO}/rulesets" --input - <<<"${payload}" >/dev/null
    fi
}

require_rulesets

# The maintainer keeps a bypass so the repository owner can still administer
# the branch; everyone else goes through a pull request.
read -r -d '' MAIN_RULESET <<JSON || true
{
  "name": "okf-toolkit: main",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [
    { "actor_type": "RepositoryRole", "actor_id": 5, "bypass_mode": "always" }
  ],
  "conditions": {
    "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": true,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false,
        "allowed_merge_methods": ["squash", "merge", "rebase"]
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": true,
        "required_status_checks": [
          { "context": "Lint" },
          { "context": "Docs" },
          { "context": "Coverage (100% gate)" },
          { "context": "Test (ubuntu-latest / stable)" },
          { "context": "Conventional Commits" },
          { "context": "Pull request title" },
          { "context": "okf validate ./knowledge" }
        ]
      }
    }
  ]
}
JSON

# Blocking branch creation is what forces contributors onto forks: without
# push access to a branch here, a pull request can only come from a fork.
#
# `release-plz-*` is excluded because the release-pr step opens its version and
# changelog pull request from a branch in this repository, pushed by
# github-actions[bot], which holds no repository role and so cannot use the
# bypass above. Excluding the namespace is narrower than granting the Actions
# app a blanket creation bypass, and it concedes nothing: a branch here still
# reaches `main` only through the pull request rules below.
read -r -d '' BRANCHES_RULESET <<JSON || true
{
  "name": "okf-toolkit: branches",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [
    { "actor_type": "RepositoryRole", "actor_id": 5, "bypass_mode": "always" }
  ],
  "conditions": {
    "ref_name": { "include": ["~ALL"], "exclude": ["refs/heads/release-plz-*"] }
  },
  "rules": [{ "type": "creation" }]
}
JSON

apply "okf-toolkit: main" "${MAIN_RULESET}"
apply "okf-toolkit: branches" "${BRANCHES_RULESET}"

echo
echo "Rulesets on ${REPO}:"
gh api "repos/${REPO}/rulesets" --jq '.[] | "  \(.name)  [\(.enforcement)]"'
echo
echo "Maintainer (${OWNER}) keeps admin bypass; everyone else must fork and open a pull request."
