#!/usr/bin/env bash
set -e

jstz_check_context() {
    if ! docker context ls --format '{{.Name}}' | grep -w "$1"; then
        echo "Docker context \"$1\" does not exist"
        exit 1
    fi
}

jstz_load_dotenv() {
    set -a 
    # shellcheck source=../.env
    source .env
    set +a
}

jstz_configure_env_network() {
    IFS= read -r -p "Enter network: " network
    sed -i"" -e "s|NETWORK=.*|NETWORK=\"$network\"|g" .env

    IFS= read -r -p "Enter tag for jstz containers: " jstz_tag
    sed -i"" -e "s|JSTZ_TAG=.*|JSTZ_TAG=\"$jstz_tag\"|g" .env
}

jstz_configure_env_rollup() {
    IFS= read -r -p "Enter jstz rollup address: " jstz_rollup_address
    sed -i"" -e "s|JSTZ_ROLLUP_ADDRESS=.*|JSTZ_ROLLUP_ADDRESS=\"$jstz_rollup_address\"|g" .env

    IFS= read -r -p "Enter jstz bridge address: " jstz_rollup_bridge_address
    sed -i"" -e "s|JSTZ_ROLLUP_BRIDGE_ADDRESS=.*|JSTZ_ROLLUP_BRIDGE_ADDRESS=\"$jstz_rollup_bridge_address\"|g" .env
}

jstz_configure_env() {
    do_configure_rollup="$1"

    if [ ! -f ".env" ]; then 
        cp .env.example .env
        echo ".env file created from .env.example"

        IFS= read -r -p "Enter operator secret key: " operator_sk
        sed -i"" -e "s|OPERATOR_SK=.*|OPERATOR_SK=\"$operator_sk\"|g" .env

        IFS= read -r -p "Enter operator address: " operator_address
        sed -i"" -e "s|OPERATOR_ADDRESS=.*|OPERATOR_ADDRESS=\"$operator_address\"|g" .env

        IFS= read -r -p "Enter docker registry: " docker_registry
        sed -i"" -e "s|DOCKER_REGISTRY=.*|DOCKER_REGISTRY=\"$docker_registry\"|g" .env

        jstz_configure_env_network

        if [ "$do_configure_rollup" = "--configure-rollup" ]; then
            jstz_configure_env_rollup
        fi
        echo ".env file setup"
    else
        echo ".env file already exists"

        IFS= read -r -p "Do you want to configure a new network (y/n): " update_network
        if [ "$update_network" = "y" ]; then
            jstz_configure_env_network
            echo "new network setup"
        fi

        if [ "$do_configure_rollup" = "--configure-rollup" ]; then    
            IFS= read -r -p "Do you want to configure a new rollup (y/n): " update_rollup
            if [ "$update_rollup" = "y" ]; then
                jstz_configure_env_rollup
                echo "new network setup"
            fi
        fi
    fi
}

jstz_deploy() {
    context="$1"
    if [ -z "$context" ]; then
        echo "Usage: start <docker context>"
        exit 1
    fi
    jstz_check_context "$1"

    jstz_configure_env

    echo "Pulling latest images from GHCR"
    docker-compose --context "$1" pull

    echo "Deploying rollup..."

    # Load the .env file to obtain the various environment variables 
    jstz_load_dotenv

    network=${NETWORK:?Unset NETWORK in .env}
    operator_sk=${OPERATOR_SK:?Unset OPERATOR_SK in .env}
    operator_address=${OPERATOR_ADDRESS:?Unset OPERATOR_ADDRESS in .env}
    jstz_tag=${JSTZ_TAG:?Unset JSTZ_TAG in .env}
    docker_registry=${DOCKER_REGISTRY:?Unset DOCKER_REGISTRY in .env}

    output=$(docker --context "$1" run -v /var/run/docker.sock:/var/run/docker.sock \
        -e NETWORK="$network" -e OPERATOR_SK="$operator_sk" -e OPERATOR_ADDRESS="$operator_address" \
        "${docker_registry}jstz-rollup:$jstz_tag" deploy)

    echo "$output"

    IFS= read -r -p "Do you want to update .env with the rollup and bridge addresses (y/n): " update_env
    if [ "$update_env" = "y" ]; then
        bridge_address=$(echo "$output" | grep -oE "KT1[a-zA-Z0-9]{33}" | uniq | tr -d '\n')
        rollup_address=$(echo "$output" | grep -oE "sr1[a-zA-Z0-9]{33}" | uniq | tr -d '\n')

        echo "Updating .env with rollup and bridge addresses"
        sed -i"" -e "s|JSTZ_ROLLUP_ADDRESS=.*|JSTZ_ROLLUP_ADDRESS=\"$rollup_address\"|g" .env
        sed -i"" -e "s|JSTZ_ROLLUP_BRIDGE_ADDRESS=.*|JSTZ_ROLLUP_BRIDGE_ADDRESS=\"$bridge_address\"|g" .env
    fi

    echo "Rollup deployed"
}

jstz_start() {
    context="$1"
    if [ -z "$context" ]; then
        echo "Usage: start <docker context>"
        exit 1
    fi
    jstz_check_context "$1"

    jstz_configure_env --configure-rollup

    echo "Stopping current containers (if running)"
    docker-compose --context "$1" down

    echo "Pulling latest images from GHCR"
    docker-compose --context "$1" pull

    echo "Spinning up containers"
    docker-compose --context "$1" up -d
}

main() {
    command="$1"
    shift 1

    case "$command" in
        start)
            jstz_start "$@"
            ;;
        deploy)
            jstz_deploy "$@"
            ;;
        *)
            echo "Usage: scripts/run-operator.sh <command>"
            echo "Commands:"
            echo "  start <docker context>"
            echo "  deploy <docker context>"
            exit 1
            ;;
    esac    
}

main "$@"