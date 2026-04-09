#!/bin/bash

# ORMDB Release Script
# Publishes to: crates.io, npm, PyPI, gem (placeholder), and Homebrew
#
# Usage:
#   ./release.sh                    # Full release
#   ./release.sh --dry-run          # Simulate without publishing
#   ./release.sh --update-brew      # Update Homebrew only (after release)

set -e
set -o pipefail

# ============================================
# Configuration
# ============================================
REPO="incredlabs/ormdb"
HOMEBREW_TAP="incredlabs/homebrew-ormdb"
SLEEP_DURATION=30

# Crates in dependency order
CRATES=(
    "ormdb-proto"
    "ormdb-lang"
    "ormdb-core"
    "ormdb-client"
    "ormdb-server"
    "ormdb-cli"
    "ormdb-gateway"
    "ormdb-studio"
)

# Colors (disabled if not a terminal)
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    BOLD='\033[1m'
    NC='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    BOLD=''
    NC=''
fi

# ============================================
# Helper Functions
# ============================================

info() {
    echo -e "${BLUE}==>${NC} ${BOLD}$1${NC}"
}

success() {
    echo -e "${GREEN}==>${NC} $1"
}

warning() {
    echo -e "${YELLOW}Warning:${NC} $1"
}

error() {
    echo -e "${RED}Error:${NC} $1" >&2
    exit "${2:-1}"
}

show_help() {
    cat <<EOF
ORMDB Release Script

Usage:
    release.sh [OPTIONS]

Options:
    --dry-run           Simulate without publishing
    --skip-crates       Skip crates.io publishing
    --skip-npm          Skip npm publishing
    --skip-pypi         Skip PyPI publishing
    --skip-brew         Skip Homebrew tap update
    --skip-tag          Skip git tag creation
    --update-brew       Only update Homebrew (run after release assets ready)
    -h, --help          Show this help message

Publishing Targets:
    - crates.io     8 Rust crates in dependency order
    - npm           @ormdb/client TypeScript package
    - PyPI          ormdb Python package
    - gem           Placeholder (Ruby client not yet available)
    - Homebrew      incredlabs/homebrew-ormdb tap

Examples:
    # Full release
    ./release.sh

    # Dry run to test
    ./release.sh --dry-run

    # Skip specific targets
    ./release.sh --skip-crates --skip-npm

    # Only update Homebrew after release assets are uploaded
    ./release.sh --update-brew
EOF
}

# ============================================
# Version Management
# ============================================

get_version() {
    local cargo_toml="${PROJECT_ROOT}/Cargo.toml"

    if [[ ! -f "$cargo_toml" ]]; then
        error "Cargo.toml not found at $cargo_toml" 1
    fi

    # Extract version from workspace.package.version
    grep -A5 '\[workspace.package\]' "$cargo_toml" | \
        grep 'version' | head -1 | \
        sed -E 's/.*version = "([^"]+)".*/\1/'
}

validate_version() {
    local version="$1"

    # Validate semver format
    if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
        error "Invalid version format: $version" 1
    fi

    # Check if tag already exists
    if git tag -l "v$version" | grep -q "v$version"; then
        error "Tag v$version already exists. Bump version in Cargo.toml first." 1
    fi
}

# ============================================
# Publishing Functions
# ============================================

publish_crates() {
    local dry_run="$1"

    echo ""
    echo "========================================"
    echo "Publishing to crates.io"
    echo "========================================"
    echo ""

    info "Crates will be published in this order:"
    for i in "${!CRATES[@]}"; do
        echo "    $((i+1)). ${CRATES[$i]}"
    done
    echo ""

    # Verify build first
    info "Running cargo check..."
    cargo check --workspace --all-features

    for crate in "${CRATES[@]}"; do
        local crate_path="${PROJECT_ROOT}/crates/${crate}"

        if [[ ! -d "$crate_path" ]]; then
            warning "Crate directory not found: $crate_path, skipping..."
            continue
        fi

        echo ""
        info "Publishing: ${crate}"

        cd "$crate_path"

        if [[ "$dry_run" == "true" ]]; then
            cargo publish --dry-run
        else
            cargo publish
            echo ""
            echo "    Waiting ${SLEEP_DURATION}s for crates.io indexing..."
            sleep ${SLEEP_DURATION}
        fi

        cd "$PROJECT_ROOT"
    done

    echo ""
    success "crates.io publishing complete"
}

publish_npm() {
    local dry_run="$1"
    local client_dir="${PROJECT_ROOT}/clients/typescript"

    echo ""
    echo "========================================"
    echo "Publishing to npm"
    echo "========================================"
    echo ""

    if [[ ! -d "$client_dir" ]]; then
        warning "TypeScript client not found at $client_dir, skipping..."
        return 0
    fi

    cd "$client_dir"

    # Check npm login
    if ! npm whoami &>/dev/null; then
        error "Not logged in to npm. Run: npm login" 1
    fi

    local npm_user
    npm_user=$(npm whoami)
    info "Logged in as: $npm_user"

    # Install dependencies
    info "Installing dependencies..."
    npm ci

    # Build the package
    info "Building TypeScript client..."
    npm run build

    # Run tests if available
    if npm run test:run --if-present &>/dev/null; then
        info "Running tests..."
        npm run test:run
    fi

    # Publish
    echo ""
    if [[ "$dry_run" == "true" ]]; then
        info "Dry run publishing..."
        npm publish --dry-run --access public
    else
        info "Publishing to npm..."
        npm publish --access public
    fi

    cd "$PROJECT_ROOT"

    echo ""
    success "Published: @ormdb/client"
}

publish_pypi() {
    local dry_run="$1"
    local client_dir="${PROJECT_ROOT}/clients/python"

    echo ""
    echo "========================================"
    echo "Publishing to PyPI"
    echo "========================================"
    echo ""

    if [[ ! -d "$client_dir" ]]; then
        warning "Python client not found at $client_dir, skipping..."
        return 0
    fi

    cd "$client_dir"

    # Check for required tools
    if ! command -v python3 &>/dev/null; then
        error "python3 is required" 1
    fi

    # Install build tools if needed
    if ! python3 -c "import hatchling" &>/dev/null; then
        info "Installing hatch..."
        pip3 install hatch
    fi

    # Clean previous builds
    rm -rf dist/ build/ *.egg-info/

    # Build with hatch
    info "Building Python package..."
    python3 -m build

    # List built artifacts
    info "Built artifacts:"
    ls -la dist/

    # Publish
    echo ""
    if [[ "$dry_run" == "true" ]]; then
        info "Dry run - would upload:"
        ls dist/
    else
        info "Publishing to PyPI..."

        # Use twine for upload
        if ! command -v twine &>/dev/null; then
            pip3 install twine
        fi

        twine upload dist/*
    fi

    cd "$PROJECT_ROOT"

    echo ""
    success "Published: ormdb"
}

publish_gem() {
    local dry_run="$1"
    local client_dir="${PROJECT_ROOT}/clients/ruby"

    echo ""
    echo "========================================"
    echo "Ruby Gem Publishing"
    echo "========================================"
    echo ""

    if [[ ! -d "$client_dir" ]]; then
        info "Ruby client not yet available"
        echo "    Location: $client_dir"
        echo "    Status: Placeholder for future implementation"
        echo ""
        echo "    When ready, this will:"
        echo "      - Build gem from ormdb.gemspec"
        echo "      - Push to RubyGems.org"
        return 0
    fi

    cd "$client_dir"

    # Find gemspec file
    local gemspec
    gemspec=$(find . -maxdepth 1 -name "*.gemspec" | head -1)

    if [[ -z "$gemspec" ]]; then
        warning "No gemspec found in $client_dir"
        cd "$PROJECT_ROOT"
        return 0
    fi

    info "Building gem from $gemspec..."
    gem build "$gemspec"

    local gem_file
    gem_file=$(ls -t *.gem 2>/dev/null | head -1)

    if [[ -z "$gem_file" ]]; then
        error "Gem build failed" 1
    fi

    if [[ "$dry_run" == "true" ]]; then
        info "Dry run - would push: $gem_file"
    else
        info "Pushing to RubyGems..."
        gem push "$gem_file"
    fi

    cd "$PROJECT_ROOT"

    echo ""
    success "Published: ormdb gem"
}

update_homebrew() {
    local version="$1"
    local dry_run="$2"

    echo ""
    echo "========================================"
    echo "Updating Homebrew Tap"
    echo "========================================"
    echo ""

    local tap_dir="/tmp/homebrew-ormdb-$$"
    local formula_file="Formula/ormdb.rb"

    # Check for gh CLI
    if ! command -v gh &>/dev/null; then
        warning "GitHub CLI (gh) not found. Install it to update Homebrew tap."
        echo "    Install: https://cli.github.com/"
        return 0
    fi

    # Clone the tap repository
    info "Cloning Homebrew tap..."
    rm -rf "$tap_dir"

    if ! git clone "https://github.com/${HOMEBREW_TAP}.git" "$tap_dir" 2>/dev/null; then
        warning "Could not clone Homebrew tap. It may not exist yet."
        echo ""
        echo "    To create the tap repository:"
        echo "      1. Create repo: https://github.com/new (name: homebrew-ormdb)"
        echo "      2. Add Formula/ormdb.rb with the Homebrew formula"
        echo "      3. Re-run: ./release.sh --update-brew"
        return 0
    fi

    cd "$tap_dir"

    # Create Formula directory if it doesn't exist
    mkdir -p Formula

    # Calculate SHA256 for each release asset
    local platforms=("darwin-x86_64" "darwin-aarch64" "linux-x86_64" "linux-aarch64")
    declare -A shasums

    info "Calculating SHA256 checksums for release assets..."
    for platform in "${platforms[@]}"; do
        local url="https://github.com/${REPO}/releases/download/v${version}/ormdb-v${version}-${platform}.tar.gz"
        echo "    Fetching: ${platform}..."

        local sha
        sha=$(curl -sL "$url" 2>/dev/null | shasum -a 256 | cut -d' ' -f1)

        if [[ -z "$sha" || "$sha" == "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" ]]; then
            warning "Could not fetch $platform release asset"
            sha="PLACEHOLDER_SHA256_${platform}"
        fi

        shasums[$platform]="$sha"
    done

    # Generate the formula
    info "Generating Homebrew formula..."
    cat > "$formula_file" <<EOF
class Ormdb < Formula
  desc "ORMDB - An embedded graph database with multi-writer support"
  homepage "https://docs.skelfresearch.com/ormdb"
  version "${version}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/${REPO}/releases/download/v${version}/ormdb-v${version}-darwin-aarch64.tar.gz"
      sha256 "${shasums[darwin-aarch64]}"
    else
      url "https://github.com/${REPO}/releases/download/v${version}/ormdb-v${version}-darwin-x86_64.tar.gz"
      sha256 "${shasums[darwin-x86_64]}"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/${REPO}/releases/download/v${version}/ormdb-v${version}-linux-aarch64.tar.gz"
      sha256 "${shasums[linux-aarch64]}"
    else
      url "https://github.com/${REPO}/releases/download/v${version}/ormdb-v${version}-linux-x86_64.tar.gz"
      sha256 "${shasums[linux-x86_64]}"
    end
  end

  def install
    bin.install "ormdb"
    bin.install "ormdb-server"
    bin.install "ormdb-gateway"
    bin.install "ormdb-studio"
  end

  test do
    system "#{bin}/ormdb", "--version"
  end
end
EOF

    if [[ "$dry_run" == "true" ]]; then
        info "Dry run - formula would be:"
        echo ""
        cat "$formula_file"
    else
        info "Committing and pushing formula..."
        git add "$formula_file"
        git commit -m "Update ormdb to v${version}"
        git push origin main
    fi

    cd "$PROJECT_ROOT"
    rm -rf "$tap_dir"

    echo ""
    success "Homebrew tap updated"
    echo ""
    echo "    Users can now install with:"
    echo "        brew tap ${HOMEBREW_TAP}"
    echo "        brew install ormdb"
}

create_git_tag() {
    local version="$1"
    local dry_run="$2"

    echo ""
    info "Creating git tag v${version}..."

    if [[ "$dry_run" == "true" ]]; then
        echo "    Dry run - would create tag: v${version}"
    else
        git tag -a "v${version}" -m "Release v${version}"
        git push origin "v${version}"
        success "Created and pushed tag: v${version}"
    fi
}

# ============================================
# Main
# ============================================

main() {
    local dry_run=false
    local skip_crates=false
    local skip_npm=false
    local skip_pypi=false
    local skip_brew=false
    local skip_git_tag=false
    local update_brew_only=false

    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dry-run)
                dry_run=true
                shift
                ;;
            --skip-crates)
                skip_crates=true
                shift
                ;;
            --skip-npm)
                skip_npm=true
                shift
                ;;
            --skip-pypi)
                skip_pypi=true
                shift
                ;;
            --skip-brew)
                skip_brew=true
                shift
                ;;
            --skip-tag)
                skip_git_tag=true
                shift
                ;;
            --update-brew)
                update_brew_only=true
                shift
                ;;
            -h|--help)
                show_help
                exit 0
                ;;
            *)
                error "Unknown option: $1. Use --help for usage information." 1
                ;;
        esac
    done

    # Change to project root
    PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
    cd "$PROJECT_ROOT"

    # Verify we're in the right directory
    if [[ ! -f "Cargo.toml" ]]; then
        error "Must be run from the ormdb project root" 1
    fi

    # Get version
    local version
    version=$(get_version)

    echo ""
    echo "========================================"
    echo "    ORMDB Release v${version}"
    echo "========================================"
    echo ""

    if [[ "$dry_run" == "true" ]]; then
        echo -e "${YELLOW}DRY RUN MODE - No actual publishing will occur${NC}"
        echo ""
    fi

    # Handle --update-brew only mode
    if [[ "$update_brew_only" == "true" ]]; then
        update_homebrew "$version" "$dry_run"
        echo ""
        echo "========================================"
        echo "    Homebrew Update Complete!"
        echo "========================================"
        exit 0
    fi

    # Validate version (skip for brew-only mode)
    if [[ "$skip_git_tag" != "true" ]]; then
        validate_version "$version"
    fi

    # Show what will be published
    echo "Publishing targets:"
    [[ "$skip_crates" != "true" ]] && echo "    - crates.io (${#CRATES[@]} crates)"
    [[ "$skip_npm" != "true" ]] && echo "    - npm (@ormdb/client)"
    [[ "$skip_pypi" != "true" ]] && echo "    - PyPI (ormdb)"
    echo "    - gem (placeholder)"
    [[ "$skip_brew" != "true" ]] && echo "    - Homebrew (after release assets ready)"
    echo ""

    # Confirmation
    if [[ "$dry_run" != "true" ]]; then
        read -p "Proceed with release v${version}? (y/N) " -n 1 -r
        echo ""
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo "Aborted."
            exit 1
        fi
    fi

    # Create git tag first
    if [[ "$skip_git_tag" != "true" ]]; then
        create_git_tag "$version" "$dry_run"
    fi

    # Publish to registries
    [[ "$skip_crates" != "true" ]] && publish_crates "$dry_run"
    [[ "$skip_npm" != "true" ]] && publish_npm "$dry_run"
    [[ "$skip_pypi" != "true" ]] && publish_pypi "$dry_run"

    # Ruby gem placeholder
    publish_gem "$dry_run"

    # Homebrew update note
    if [[ "$skip_brew" != "true" ]]; then
        echo ""
        echo "========================================"
        info "Homebrew Update"
        echo "========================================"
        echo ""
        echo "    After GitHub Actions builds and uploads release assets,"
        echo "    run the following to update the Homebrew tap:"
        echo ""
        echo "        ./scripts/release.sh --update-brew"
        echo ""
    fi

    echo ""
    echo "========================================"
    echo "    Release v${version} Complete!"
    echo "========================================"
    echo ""
    echo "Published to:"
    [[ "$skip_crates" != "true" ]] && echo "    - crates.io (${#CRATES[@]} crates)"
    [[ "$skip_npm" != "true" ]] && echo "    - npm (@ormdb/client)"
    [[ "$skip_pypi" != "true" ]] && echo "    - PyPI (ormdb)"
    echo ""
    echo "Next steps:"
    echo "    1. Wait for GitHub Actions to build release binaries"
    echo "    2. Run: ./scripts/release.sh --update-brew"
    echo "    3. Announce release on social channels"
    echo ""
    echo "Links:"
    echo "    - https://crates.io/crates/ormdb"
    echo "    - https://www.npmjs.com/package/@ormdb/client"
    echo "    - https://pypi.org/project/ormdb/"
    echo "    - https://github.com/${REPO}/releases/tag/v${version}"
    echo ""
}

main "$@"
