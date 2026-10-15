#! /bin/sh
# A hook script to ensure files have proper end-of-line characters.
# This script checks for files staged for commit and ensures they end with a newline character.
# If a file does not end with a newline, it appends one.
# Binary files and deleted files are skipped.

# Handle filenames containing spaces
IFS="
"

# Paths this repo vendors rather than authors are left untouched: see
# .hooks/ignore. Reformatting vendored content breaks provenance tracking.
hooks_ignored() {
    [ -f .hooks/ignore ] || return 1
    while IFS= read -r pattern; do
        case "$pattern" in ''|\#*) continue ;; esac
        # shellcheck disable=SC2254  # the pattern is meant to glob
        case "$1" in $pattern) return 0 ;; esac
    done < .hooks/ignore
    return 1
}

# Get the list of staged files (added/copied/modified only, skip deletions)
STAGED_FILES=$(git diff --cached --name-only --diff-filter=ACM)

# Loop through each staged file
for FILE in $STAGED_FILES; do
    if hooks_ignored "$FILE"; then
        continue
    fi
    # Check if the file exists, is a regular file and is a text file (grep -qI skips binaries)
    if [ -f "$FILE" ] && grep -qI '' "$FILE"; then
        # Check if the last character of the file is a newline
        if [ -n "$(tail -c1 "$FILE" | tr -d '\n')" ]; then
            echo "Fixing end-of-line for $FILE"
            # Append a newline character to the file
            echo "" >> "$FILE"
            # Stage the modified file
            git add "$FILE"
        fi
    fi
done

echo "[End-of-Line Fixer] All done!"

exit 0
