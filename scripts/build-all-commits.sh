#!/bin/bash

# Script to build all commits in a repository
# Goes through commits from initial to master, checks out magic.rs from master, and attempts to build

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Starting build process for all commits...${NC}"

# Save current branch
ORIGINAL_BRANCH=$(git symbolic-ref --short HEAD 2>/dev/null || echo "detached")
echo -e "${YELLOW}Original branch: $ORIGINAL_BRANCH${NC}"

# Get all commit hashes from oldest to newest (reverse chronological order)
COMMITS=$(git rev-list --reverse HEAD)

# Count total commits
TOTAL_COMMITS=$(echo "$COMMITS" | wc -l)
CURRENT=0

echo -e "${GREEN}Found $TOTAL_COMMITS commits to process${NC}"

# Create target directory if it doesn't exist
mkdir -p target/release

for commit in $COMMITS; do
    CURRENT=$((CURRENT + 1))
    echo -e "\n${YELLOW}=== Processing commit $CURRENT/$TOTAL_COMMITS: $commit ===${NC}"
    
    # Checkout the commit
    echo "Checking out commit $commit..."
    git checkout "$commit" --quiet
    
    # Check out the magic.rs file from master
    echo "Checking out src/utils/magic.rs from master..."
    git checkout master -- src/utils/magic.rs 2>/dev/null || {
        echo -e "${RED}Warning: Could not checkout src/utils/magic.rs from master${NC}"
    }
    
    # Attempt to build
    echo "Attempting to build..."
    if cargo build --release 2>/dev/null; then
        # If build succeeded, try to rename the binary (try both possible names)
        if [ -f "target/release/Prokopakop" ]; then
            mv "target/release/Prokopakop" "target/release/prokopakop-$commit" 2>/dev/null || {
                echo -e "${RED}Warning: Could not rename binary Prokopakop for commit $commit${NC}"
            }
            echo -e "${GREEN}✓ Build successful for commit $commit (found Prokopakop)${NC}"
        elif [ -f "target/release/prokopakop" ]; then
            mv "target/release/prokopakop" "target/release/prokopakop-$commit" 2>/dev/null || {
                echo -e "${RED}Warning: Could not rename binary prokopakop for commit $commit${NC}"
            }
            echo -e "${GREEN}✓ Build successful for commit $commit (found prokopakop)${NC}"
        else
            echo -e "${RED}✗ Build succeeded but binary not found (tried both Prokopakop and prokopakop) for commit $commit${NC}"
        fi
    else
        echo -e "${RED}✗ Build failed for commit $commit (ignored)${NC}"
    fi
done

# Return to original branch
echo -e "\n${YELLOW}Returning to original branch/state...${NC}"
if [ "$ORIGINAL_BRANCH" = "detached" ]; then
    # If we were in detached HEAD state, we can't easily return to the exact state
    echo -e "${YELLOW}Warning: Was in detached HEAD state. Checking out master.${NC}"
    git checkout master --quiet
else
    git checkout "$ORIGINAL_BRANCH" --quiet
fi

echo -e "\n${GREEN}=== Build process complete! ===${NC}"
echo -e "${GREEN}Built binaries are in target/release/ with names like prokopakop-<commit-hash>${NC}"

# List the built binaries
echo -e "\n${YELLOW}Built binaries:${NC}"
ls -la target/release/prokopakop-* 2>/dev/null || echo "No binaries were successfully built."
