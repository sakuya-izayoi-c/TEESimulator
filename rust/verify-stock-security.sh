#!/bin/sh
# Check the *built working tree*, not just the list of patches. An incremental
# checkout may still contain a patch that was removed from the superproject.
# These are the reviewed AOSP cfeefc94 blobs after kmr-ta-seclevel.patch.
# Re-evaluate on a KeyMint pin change; never reset a user's submodule here.
set -eu
check_blob() {
  actual=$(git -C "$keymint" hash-object --no-filters -- "$2")
  if [ "$actual" != "$1" ]; then
    printf >&2 'Unexpected KeyMint security source: %s. Use a clean pinned submodule; no files were reset.\n' "$2"
    exit 1
  fi
}
keymint=${1:?pass the patched KeyMint working directory}
check_blob 4975798ae6ceb561b020051d45438aca2f62c7f3 ta/src/operation.rs
check_blob f4d3310f2248ee6f32e7da57286be164c419afae ta/src/keys.rs
check_blob 5f17d72ffb30767a61bb45ba51bb58b930b3244b ta/src/lib.rs
