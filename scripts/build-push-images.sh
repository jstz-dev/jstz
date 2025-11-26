#!/bin/bash
set -euo pipefail

# JSTZ Build and Push Script
# Builds and pushes container images and kernels to GCP Artifact Registry
#
# Usage: ./build-push-images.sh [COMPONENT] [OPTIONS]
#
# Components: all, jstz-node, oracle-node, kernel-wasm, kernel-riscv, kernel-lightweight, kernels
# Options: --tag TAG --project ID --region REGION --registry NAME --help
#
# What gets published:
# - jstz-kernel-{TYPE}:TAG           - Raw kernel binary (Debian Bookworm)
# - jstz-kernel-artifacts-{TYPE}:TAG - Installer + preimages (Debian Bookworm)
#
# The artifacts image contains:
#   /kernel-artifacts/kernel_installer.hex
#   /kernel-artifacts/parameters_ty.json
#   /kernel-artifacts/preimages/
#
# Note: All kernel builds require Nix to apply float instruction patches

# Configuration defaults
GCP_PROJECT="${GCP_PROJECT:-jstz-dev-dbc1}"
GCP_REGION="${GCP_REGION:-europe-west2}"
REGISTRY_NAME="${REGISTRY_NAME:-riscvnet}"
TAG="latest"

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Component to build (will be set after parsing arguments)
COMPONENT="all"

show_usage() {
  echo "Usage: $0 [COMPONENT] [OPTIONS]"
  echo ""
  echo "Components:"
  echo "  all              Build and push all components"
  echo "  jstz-node        Build and push JSTZ node"
  echo "  oracle-node      Build and push Oracle node"
  echo "  bridge-contracts Build and push bridge contracts (exchanger, native bridge)"
  echo "  kernel-wasm      Build and publish WASM kernel (requires Nix)"
  echo "  kernel-riscv     Build and publish RISC-V kernel (requires Nix)"
  echo "  kernel-lightweight Build and publish lightweight RISC-V kernel (requires Nix)"
  echo "  kernels          Build and publish all kernels (requires Nix)"
  echo ""
  echo "Options:"
  echo "  --tag TAG        Image tag (default: latest)"
  echo "  --project ID     GCP project ID (default: jstz-dev-dbc1)"
  echo "  --region REGION  GCP region (default: europe-west2)"
  echo "  --registry NAME  Registry name (default: riscvnet)"
  echo "  --help           Show this help"
  echo ""
  echo "Note: Kernel builds require Nix to be installed for float instruction support."
  echo "Install Nix: https://nixos.org/download.html"
  exit 0
}

# Parse arguments
while [[ $# -gt 0 ]]; do
  case $1 in
  --tag)
    TAG="$2"
    shift 2
    ;;
  --project)
    GCP_PROJECT="$2"
    shift 2
    ;;
  --region)
    GCP_REGION="$2"
    shift 2
    ;;
  --registry)
    REGISTRY_NAME="$2"
    shift 2
    ;;
  --help)
    show_usage
    ;;
  *)
    # First non-option argument is the component
    if [[ $COMPONENT == "all" ]]; then
      COMPONENT="$1"
    else
      echo "Unknown option: $1"
      show_usage
    fi
    shift
    ;;
  esac
done

REGISTRY_URL="${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${REGISTRY_NAME}"

# Multi-platform support (AMD64 for GKE, ARM64 for local Mac)
PLATFORMS="linux/amd64,linux/arm64"

setup_buildx() {
  # Create or use existing buildx builder for multi-platform builds
  if ! docker buildx inspect jstz-builder &>/dev/null; then
    echo "Creating multi-platform builder..."
    docker buildx create --name jstz-builder --driver docker-container --use
  else
    docker buildx use jstz-builder
  fi
}

authenticate_gcp() {
  gcloud auth configure-docker ${GCP_REGION}-docker.pkg.dev --quiet

  if ! gcloud artifacts repositories describe ${REGISTRY_NAME} \
    --project=${GCP_PROJECT} \
    --location=${GCP_REGION} &>/dev/null; then
    gcloud artifacts repositories create ${REGISTRY_NAME} \
      --repository-format=docker \
      --location=${GCP_REGION} \
      --project=${GCP_PROJECT} \
      --description="JSTZ RISC-V network containers"
  fi
}

build_and_push_container() {
  local NAME=$1
  local DOCKERFILE=$2
  local CONTEXT=$3
  local IMAGE_TAG=$4

  docker buildx build \
    --platform "${PLATFORMS}" \
    --tag "${REGISTRY_URL}/${NAME}:${IMAGE_TAG}" \
    --file "${DOCKERFILE}" \
    --push \
    "${CONTEXT}"
}

generate_kernel_artifacts() {
  local KERNEL_TYPE=$1
  local KERNEL_PATH=$2
  local ARTIFACTS_DIR="${PROJECT_ROOT}/target/kernel-artifacts-${KERNEL_TYPE}"

  echo "Generating kernel installer and preimages for ${KERNEL_TYPE}..."

  # Note: We don't copy to jstzd resources anymore, only to the build script runner

  # Create output directory
  mkdir -p "${ARTIFACTS_DIR}"

  # Run build.rs as a standalone program using a temporary Cargo project
  # This avoids needing to compile the full jstzd binary
  local BUILD_SCRIPT_DIR="${PROJECT_ROOT}/target/build-script-runner-${KERNEL_TYPE}"
  rm -rf "${BUILD_SCRIPT_DIR}"
  mkdir -p "${BUILD_SCRIPT_DIR}/src"

  # Copy build.rs and build_config.rs
  cp "${PROJECT_ROOT}/crates/jstzd/build.rs" "${BUILD_SCRIPT_DIR}/src/main.rs"
  cp "${PROJECT_ROOT}/crates/jstzd/build_config.rs" "${BUILD_SCRIPT_DIR}/src/"

  # Copy required resource files
  mkdir -p "${BUILD_SCRIPT_DIR}/resources/jstz_rollup"
  mkdir -p "${BUILD_SCRIPT_DIR}/resources/bootstrap_account"
  cp "${KERNEL_PATH}" "${BUILD_SCRIPT_DIR}/resources/jstz_rollup/jstz_kernel.wasm"
  cp "${PROJECT_ROOT}/crates/jstzd/resources/jstz_rollup/parameters_ty.json" "${BUILD_SCRIPT_DIR}/resources/jstz_rollup/"

  # Fetch bootstrap accounts from GCP Secret Manager instead of using local file
  # This ensures we use the same accounts that testnet uses
  echo "Fetching bootstrap accounts from GCP Secret Manager..."
  if ! gcloud secrets versions access latest --secret="riscvnet-bootstrap-accounts" \
    --project="${GCP_PROJECT}" \
    >"${BUILD_SCRIPT_DIR}/resources/bootstrap_account/accounts.json" 2>/dev/null; then
    echo "Warning: Failed to fetch accounts from GCP Secret Manager, falling back to local file"
    cp "${PROJECT_ROOT}/crates/jstzd/resources/bootstrap_account/accounts.json" \
      "${BUILD_SCRIPT_DIR}/resources/bootstrap_account/" 2>/dev/null || {
      echo "Error: No accounts.json found locally and GCP fetch failed"
      return 1
    }
  else
    echo "Successfully fetched accounts from GCP Secret Manager"
  fi

  # Copy Cargo.lock to ensure exact dependency versions match the workspace
  cp "${PROJECT_ROOT}/Cargo.lock" "${BUILD_SCRIPT_DIR}/Cargo.lock"

  # Create a minimal Cargo.toml for the build script
  cat >"${BUILD_SCRIPT_DIR}/Cargo.toml" <<EOF
[package]
name = "kernel-installer-generator"
version = "0.1.0"
edition = "2021"

# Empty workspace table to prevent inheriting parent workspace
[workspace]

[[bin]]
name = "kernel-installer-generator"
path = "src/main.rs"

[dependencies]
anyhow = "1.0"
bincode = "=2.0.0-rc.3"
hex = "0.4"
serde_json = "1.0"
tezos_crypto_rs = { git = "https://github.com/jstz-dev/tezos", rev = "0e21f47f1be4564f95c61a6cf32d02a03e87180c" }
tezos-smart-rollup = { git = "https://github.com/jstz-dev/tezos", rev = "0e21f47f1be4564f95c61a6cf32d02a03e87180c" }
tezos-smart-rollup-installer = { git = "https://github.com/jstz-dev/tezos", rev = "0e21f47f1be4564f95c61a6cf32d02a03e87180c" }
tezos-smart-rollup-installer-config = { git = "https://github.com/jstz-dev/tezos", rev = "0e21f47f1be4564f95c61a6cf32d02a03e87180c" }
jstz_kernel = { path = "${PROJECT_ROOT}/crates/kernels/jstz_kernel" }
jstz_crypto = { path = "${PROJECT_ROOT}/crates/jstz_crypto" }

[patch.crates-io]
boa_ast = { git = "https://github.com/trilitech/boa.git", branch = "ajob410@fix/remove-wasm-bindgen-from-time" }
boa_engine = { git = "https://github.com/trilitech/boa.git", branch = "ajob410@fix/remove-wasm-bindgen-from-time" }
boa_gc = { git = "https://github.com/trilitech/boa.git", branch = "ajob410@fix/remove-wasm-bindgen-from-time" }
boa_interner = { git = "https://github.com/trilitech/boa.git", branch = "ajob410@fix/remove-wasm-bindgen-from-time" }
boa_macros = { git = "https://github.com/trilitech/boa.git", branch = "ajob410@fix/remove-wasm-bindgen-from-time" }
boa_parser = { git = "https://github.com/trilitech/boa.git", branch = "ajob410@fix/remove-wasm-bindgen-from-time" }
boa_profiler = { git = "https://github.com/trilitech/boa.git", branch = "ajob410@fix/remove-wasm-bindgen-from-time" }
EOF

  # Build and run the standalone program
  cd "${BUILD_SCRIPT_DIR}"
  # OUT_DIR is where build.rs writes files during compilation
  # KERNEL_DEST_DIR is where we want the final artifacts copied to
  # CARGO_MANIFEST_DIR is needed for relative paths in build.rs
  mkdir -p "${BUILD_SCRIPT_DIR}/out"
  OUT_DIR="${BUILD_SCRIPT_DIR}/out" \
    CARGO_MANIFEST_DIR="${BUILD_SCRIPT_DIR}" \
    KERNEL_DEST_DIR="${ARTIFACTS_DIR}" \
    cargo run --release

  # Cleanup
  rm -rf "${BUILD_SCRIPT_DIR}"

  echo "Kernel artifacts generated at: ${ARTIFACTS_DIR}"
  ls -lah "${ARTIFACTS_DIR}"
}

publish_kernel_artifacts() {
  local KERNEL_TYPE=$1
  local IMAGE_TAG=$2
  local ARTIFACTS_DIR="${PROJECT_ROOT}/target/kernel-artifacts-${KERNEL_TYPE}"

  if [ ! -d "${ARTIFACTS_DIR}" ]; then
    echo "Kernel artifacts not found: ${ARTIFACTS_DIR}"
    return 1
  fi

  # Create a clean build directory
  local BUILD_DIR="${PROJECT_ROOT}/target/docker-build-${KERNEL_TYPE}"
  rm -rf "${BUILD_DIR}"
  mkdir -p "${BUILD_DIR}"

  # Copy artifacts to build dir
  cp -r "${ARTIFACTS_DIR}"/* "${BUILD_DIR}/"

  # Create Dockerfile that includes installer + preimages
  # Use Debian Bookworm to match jstzd/jstz-node runtime environment
  cat >"${BUILD_DIR}/Dockerfile" <<EOF
FROM debian:bookworm-20250520-slim
RUN apt-get update && apt-get install -y --no-install-recommends bash && rm -rf /var/lib/apt/lists/*
COPY kernel_installer.hex /kernel-artifacts/kernel_installer.hex
COPY parameters_ty.json /kernel-artifacts/parameters_ty.json
COPY preimages/ /kernel-artifacts/preimages/
EOF

  # Build and push multi-platform image
  docker buildx build \
    --platform "${PLATFORMS}" \
    --tag "${REGISTRY_URL}/jstz-kernel-artifacts-${KERNEL_TYPE}:${IMAGE_TAG}" \
    --push \
    "${BUILD_DIR}"

  # Cleanup
  rm -rf "${BUILD_DIR}"
}

publish_kernel() {
  local KERNEL_TYPE=$1
  local KERNEL_PATH=$2
  local IMAGE_TAG=$3

  if [ ! -f "${KERNEL_PATH}" ]; then
    echo "Kernel not found: ${KERNEL_PATH}"
    return 1
  fi

  # Create a clean build directory
  local BUILD_DIR="${PROJECT_ROOT}/target/docker-build-kernel-${KERNEL_TYPE}"
  rm -rf "${BUILD_DIR}"
  mkdir -p "${BUILD_DIR}"

  cp "${KERNEL_PATH}" "${BUILD_DIR}/kernel"

  # Use Debian Bookworm to match jstzd/jstz-node runtime environment
  cat >"${BUILD_DIR}/Dockerfile" <<EOF
FROM debian:bookworm-20250520-slim
RUN apt-get update && apt-get install -y --no-install-recommends bash && rm -rf /var/lib/apt/lists/*
COPY kernel /kernel
EOF

  # Build and push multi-platform image
  docker buildx build \
    --platform "${PLATFORMS}" \
    --tag "${REGISTRY_URL}/jstz-kernel-${KERNEL_TYPE}:${IMAGE_TAG}" \
    --push \
    "${BUILD_DIR}"

  # Cleanup
  rm -rf "${BUILD_DIR}"
}

# Build specific component
build_jstz_node() {
  build_and_push_container \
    "jstz-node" \
    "${PROJECT_ROOT}/crates/jstz_node/Dockerfile" \
    "${PROJECT_ROOT}" \
    "${TAG}"
}

build_oracle_node() {
  build_and_push_container \
    "jstz-oracle-node" \
    "${PROJECT_ROOT}/crates/jstz_oracle_node/Dockerfile" \
    "${PROJECT_ROOT}" \
    "${TAG}"
}

build_bridge_contracts() {
  echo "Publishing bridge contracts to Artifact Registry..."

  local CONTRACTS_DIR="${PROJECT_ROOT}/contracts"
  local CONTRACTS_REPO="riscvnet-contracts"
  local CONTRACTS_VERSION="v1.0.0"

  # Verify contracts exist
  if [ ! -f "${CONTRACTS_DIR}/exchanger.tz" ] || [ ! -f "${CONTRACTS_DIR}/jstz_native_bridge.tz" ]; then
    echo "Error: Bridge contract files not found in ${CONTRACTS_DIR}"
    return 1
  fi

  # Create generic artifacts repository if it doesn't exist
  echo "Ensuring generic artifacts repository exists..."
  if ! gcloud artifacts repositories describe ${CONTRACTS_REPO} \
    --project=${GCP_PROJECT} \
    --location=${GCP_REGION} &>/dev/null; then
    echo "Creating generic artifacts repository: ${CONTRACTS_REPO}"
    gcloud artifacts repositories create ${CONTRACTS_REPO} \
      --repository-format=generic \
      --location=${GCP_REGION} \
      --project=${GCP_PROJECT} \
      --description="JSTZ bridge contracts (Michelson .tz files)"
  else
    echo "Repository ${CONTRACTS_REPO} already exists"
  fi

  # Upload contracts with static version
  echo "Uploading exchanger.tz..."
  gcloud artifacts generic upload \
    --project=${GCP_PROJECT} \
    --location=${GCP_REGION} \
    --repository=${CONTRACTS_REPO} \
    --package=exchanger \
    --version=${CONTRACTS_VERSION} \
    --source=${CONTRACTS_DIR}/exchanger.tz

  echo "Uploading jstz_native_bridge.tz..."
  gcloud artifacts generic upload \
    --project=${GCP_PROJECT} \
    --location=${GCP_REGION} \
    --repository=${CONTRACTS_REPO} \
    --package=jstz_native_bridge \
    --version=${CONTRACTS_VERSION} \
    --source=${CONTRACTS_DIR}/jstz_native_bridge.tz

  echo ""
  echo "✓ Bridge contracts published to Artifact Registry:"
  echo "  Repository: ${GCP_REGION}-generic.pkg.dev/${GCP_PROJECT}/${CONTRACTS_REPO}"
  echo "  Exchanger: exchanger/${CONTRACTS_VERSION}"
  echo "  Native Bridge: jstz_native_bridge/${CONTRACTS_VERSION}"
  echo ""
  echo "To download:"
  echo "  gcloud artifacts generic download --location=${GCP_REGION} --project=${GCP_PROJECT} --repository=${CONTRACTS_REPO} --package=exchanger --version=${CONTRACTS_VERSION} --destination=."
}

build_kernel_wasm() {
  echo "Building WASM kernel with Nix..."
  cd "${PROJECT_ROOT}"

  # Check if nix is available
  if ! command -v nix &>/dev/null; then
    echo "Error: WASM kernel builds require Nix to be installed for float support."
    echo "Install Nix: https://nixos.org/download.html"
    exit 1
  fi

  # Use Nix to build kernel (applies the float patches automatically)
  nix develop --command bash -c "make build-jstzd-kernel"

  local KERNEL_PATH="${PROJECT_ROOT}/crates/jstzd/resources/jstz_rollup/jstz_kernel.wasm"

  # Generate installer and preimages using build.rs
  generate_kernel_artifacts "wasm" "${KERNEL_PATH}"

  # Publish both raw kernel and artifacts
  publish_kernel "wasm" "${KERNEL_PATH}" "${TAG}"
  publish_kernel_artifacts "wasm" "${TAG}"
}

build_kernel_riscv() {
  echo "Building RISC-V kernel with Nix..."
  cd "${PROJECT_ROOT}"

  # Check if nix is available
  if ! command -v nix &>/dev/null; then
    echo "Error: RISC-V kernels require Nix to be installed."
    echo "Install Nix: https://nixos.org/download.html"
    exit 1
  fi

  nix develop --command bash -c "make riscv-pvm-kernel"

  local KERNEL_PATH="${PROJECT_ROOT}/target/riscv64gc-unknown-linux-musl/release/kernel-executable"

  # Generate installer and preimages using build.rs
  generate_kernel_artifacts "riscv" "${KERNEL_PATH}"

  # Publish both raw kernel and artifacts
  publish_kernel "riscv" "${KERNEL_PATH}" "${TAG}"
  publish_kernel_artifacts "riscv" "${TAG}"
}

build_kernel_lightweight() {
  echo "Building lightweight RISC-V kernel with Nix..."
  cd "${PROJECT_ROOT}"

  # Check if nix is available
  if ! command -v nix &>/dev/null; then
    echo "Error: RISC-V kernels require Nix to be installed."
    echo "Install Nix: https://nixos.org/download.html"
    exit 1
  fi

  nix develop --command bash -c "make build-lightweight-kernel"

  local KERNEL_PATH="${PROJECT_ROOT}/target/riscv64gc-unknown-linux-musl/release/lightweight-kernel-executable"

  # Generate installer and preimages using build.rs
  generate_kernel_artifacts "lightweight" "${KERNEL_PATH}"

  # Publish both raw kernel and artifacts
  publish_kernel "lightweight" "${KERNEL_PATH}" "${TAG}"
  publish_kernel_artifacts "lightweight" "${TAG}"
}

build_all_kernels() {
  build_kernel_wasm || true
  build_kernel_riscv || true
  build_kernel_lightweight || true
}

build_all() {
  build_jstz_node
  build_oracle_node
  build_all_kernels
}

print_summary() {
  echo "Built: ${REGISTRY_URL}/${COMPONENT}:${TAG}"
}

# Main execution
main() {
  # Setup multi-platform builder
  setup_buildx

  # Always authenticate
  authenticate_gcp

  # Build requested component
  case "$COMPONENT" in
  all)
    build_all
    ;;
  jstz-node)
    build_jstz_node
    ;;
  oracle-node)
    build_oracle_node
    ;;
  bridge-contracts)
    build_bridge_contracts
    ;;
  kernel-wasm)
    build_kernel_wasm
    ;;
  kernel-riscv)
    build_kernel_riscv
    ;;
  kernel-lightweight)
    build_kernel_lightweight
    ;;
  kernels)
    build_all_kernels
    ;;
  *)
    echo "Error: Unknown component: $COMPONENT"
    show_usage
    ;;
  esac
}

# Run main
main
