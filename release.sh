#!/usr/bin/env bash
set -euo pipefail

DRY_RUN=false
if [ "${1:-}" = "--dry-run" ]; then
    DRY_RUN=true
    echo "=== DRY RUN MODE ==="
fi

# 1. Read version from Cargo.toml
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
if [ -z "$VERSION" ] || ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+'; then
    echo "Error: could not parse a valid version from Cargo.toml (got: '$VERSION')" >&2
    exit 1
fi
TAG="v$VERSION"
echo "Version: $VERSION (tag: $TAG)"

# 2. Verify clean working tree
if [ -n "$(git status --porcelain)" ]; then
    echo "Error: working tree is not clean" >&2
    exit 1
fi
echo "Working tree is clean"

# 3. Verify HEAD is pushed to a remote branch
if [ -z "$(git branch -r --contains HEAD)" ]; then
    echo "Error: HEAD commit is not on any remote branch" >&2
    exit 1
fi
echo "HEAD is pushed to remote"

# 4. Verify CI passed
SHA=$(git rev-parse HEAD)
RESULTS=$(gh run list --workflow=ci.yml --commit="$SHA" --status=completed --json conclusion --jq '.[].conclusion')
if [ -z "$RESULTS" ]; then
    echo "Error: no completed CI runs found for commit $SHA" >&2
    exit 1
fi
if echo "$RESULTS" | grep -v '^$' | grep -qv "success"; then
    echo "Error: CI has not passed on commit $SHA" >&2
    exit 1
fi
echo "CI passed on $SHA"

# 5. Verify tag doesn't already exist
if [ -n "$(git tag -l "$TAG")" ]; then
    echo "Error: tag $TAG already exists" >&2
    exit 1
fi
echo "Tag $TAG is available"

if [ "$DRY_RUN" = true ]; then
    echo "=== Dry run: running cargo publish --dry-run ==="
    cargo publish --dry-run
    echo "=== Dry run complete ==="
    exit 0
fi

# 6. Create and push annotated tag (signed if git config has signing set up)
git tag -a "$TAG" -m "Release $TAG"
git push origin "$TAG"
echo "Tag $TAG created and pushed"

# 7. Create GitHub release
gh release create "$TAG" --generate-notes
echo "GitHub release $TAG created"

# 8. Publish to crates.io
cargo publish
echo "Published $VERSION to crates.io"
