#!/bin/bash
set -euo pipefail

# JSTZ Build and Push Script
# Builds and pushes container images and kernels to GCP Artifact Registry
#
# Usage: ./build-push-images.sh [COMPONENT] [OPTIONS]
#
# Components: all, jstz-node, oracle-node, kernel-wasm, kernel-riscv, kernel-lightweight, kernels
# Options: --tag TAG --project ID --region REGION --registry NAME --help

# Configuration defaults
GCP_PROJECT="${GCP_PROJECT:-jstz-dev-dbc1}"
GCP_REGION="${GCP_REGION:-europe-west2}"
REGISTRY_NAME="${REGISTRY_NAME:-riscvnet}"
TAG="latest"

# Get script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Component to build
COMPONENT="${1:-all}"

show_usage() {
  echo "Usage: $0 [COMPONENT] [OPTIONS]"
  echo ""
  echo "Components:"
  echo "  all              Build and push all components"
  echo "  jstz-node        Build and push JSTZ node"
  echo "  oracle-node      Build and push Oracle node"
  echo "  kernel-wasm      Build and publish WASM kernel"
  echo "  kernel-riscv     Build and publish RISC-V kernel"
  echo "  kernel-lightweight Build and publish lightweight RISC-V kernel"
  echo "  kernels          Build and publish all kernels"
  echo ""
  echo "Options:"
  echo "  --tag TAG        Image tag (default: latest)"
  echo "  --project ID     GCP project ID (default: jstz-dev-dbc1)"
  echo "  --region REGION  GCP region (default: europe-west2)"
  echo "  --registry NAME  Registry name (default: riscvnet)"
  echo "  --help           Show this help"
  exit 0
}

# Parse arguments
shift || true
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
    echo "Unknown option: $1"
    show_usage
    ;;
  esac
done

REGISTRY_URL="${GCP_REGION}-docker.pkg.dev/${GCP_PROJECT}/${REGISTRY_NAME}"

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

  docker build -f "${DOCKERFILE}" -t "${REGISTRY_URL}/${NAME}:${IMAGE_TAG}" "${CONTEXT}"
  docker push "${REGISTRY_URL}/${NAME}:${IMAGE_TAG}"
}

publish_kernel() {
  local KERNEL_TYPE=$1
  local KERNEL_PATH=$2
  local IMAGE_TAG=$3

  if [ ! -f "${KERNEL_PATH}" ]; then
    echo "Kernel not found: ${KERNEL_PATH}"
    return 1
  fi

  cat >/tmp/Dockerfile.kernel <<EOF
FROM scratch
COPY $(basename ${KERNEL_PATH}) /kernel
EOF

  cp "${KERNEL_PATH}" /tmp/
  docker build -f /tmp/Dockerfile.kernel -t "${REGISTRY_URL}/jstz-kernel-${KERNEL_TYPE}:${IMAGE_TAG}" /tmp/
  docker push "${REGISTRY_URL}/jstz-kernel-${KERNEL_TYPE}:${IMAGE_TAG}"
  rm -f /tmp/Dockerfile.kernel /tmp/$(basename ${KERNEL_PATH})
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

build_kernel_wasm() {
  local KERNEL_PATH="${PROJECT_ROOT}/target/wasm32-unknown-unknown/release/jstz_kernel.wasm"
  publish_kernel "wasm" "${KERNEL_PATH}" "${TAG}"
}

build_kernel_riscv() {
  local KERNEL_PATH="${PROJECT_ROOT}/target/riscv64gc-unknown-linux-musl/release/kernel-executable"
  publish_kernel "riscv" "${KERNEL_PATH}" "${TAG}"
}

build_kernel_lightweight() {
  local KERNEL_PATH="${PROJECT_ROOT}/target/riscv64gc-unknown-linux-musl/release/lightweight-kernel-executable"
  publish_kernel "lightweight" "${KERNEL_PATH}" "${TAG}"
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
