#!/bin/bash

cd "$(dirname "$0")" || exit

# Enable cross-compile support if configured
_CC_ENV="$(dirname "$0")/../../../scripts/cross-compile-env.sh"
if [ -f "$_CC_ENV" ]; then
    # shellcheck disable=SC1090
    source "$_CC_ENV"
else
    echo "Not using cross-compilation (${_CC_ENV} does not exist)"
fi

# Check if DIST is set by environment variable
if [ -n "$DIST" ]; then
    echo "Using distribution from DIST environment variable: $DIST"
    DIST_ARG="--dist=$DIST"
else
    echo "No DIST environment variable set, using sbuild default"
    DIST_ARG=""
fi

if [ -f target ]; then
    echo "Removing previous build target symlink/file"
    rm -f target
fi

sbuild --chroot-mode=unshare \
       --enable-network \
       --no-clean-source \
       --verbose \
       "$DIST_ARG" 

# Step 4: Clean up build artifacts
echo "Cleaning up build artifacts..."
cd ..
rm -f -- *.build *.changes *.dsc *.tar.xz *.buildinfo
echo "Build artifacts cleaned up"

echo "Package built successfully"
echo "Built packages:"
ls -la -- *.deb 2>/dev/null
