#!/usr/bin/env bash

{ # this ensures the entire script is downloaded

version="v0.1.0-alpha.0"
network="weeklynet-2024-03-06"
container="ghcr.io/trilitech/jstz-cli:$version"
jstz_home="$HOME/.jstz"

# (--rm): remove the container after it exits
# (-v): the container mounts the following volumes:
#  - /tmp for temporary files
#  - $HOME/.jstz for configuration
#  - $PWD for the current working directory
# (-it): for interactive mode & tty support
# (--network=host): use the host's network stack
# (-w): set the working directory to the current working directory
shell_alias="alias jstz='docker run --rm -v \"/tmp:/tmp\" -v \"$jstz_home:/root/.jstz\" -v \"\$PWD:\$PWD\" -w \"\$PWD\" --network=host -it $container'"

jstz_download() {
    # Check if Docker is installed
    if ! command -v docker &> /dev/null; then
        echo "Docker is not installed. Please install Docker and try again."
        exit 1
    fi

    # Pull the Docker CLI container from GitHub Container Registry
    echo "Pulling jstz-cli docker container from GHCR..."
    DOCKER_CLI_HINTS=false docker pull "$container"
    echo ""
}

jstz_configure() {
    if [ -d "$jstz_home" ]; then
        echo "Configuration already exists. Skipping..."
    else
        echo -n "Configuring jstz..."

        mkdir -p "$jstz_home"
        cat >"$jstz_home/config.json" << EOF
{
  "default_network": "dev",
  "networks": {
    "weeklynet": {
      "octez_node_rpc_endpoint": "http://rpc.$network.teztnets.com",
      "jstz_node_endpoint": "http://34.39.12.211:8933"
    }
  }
}
EOF

        echo "done"
    fi

    echo "Configuring \`jstz\` alias..."

    shell=$(basename "$SHELL")
    case "$shell" in
        "bash")
            shellrc="$HOME/.bashrc"
            ;;
        "zsh")
            shellrc="$HOME/.zshrc"
            ;;
        *)
            cat 1>&2 << EOF
Unsupported shell: $shell. 
Please manually add the alias to your shell's configuration file.
    alias jstz='docker run -v "\$HOME/.jstz:/root/.jstz" -v "\$PWD:\$PWD" -it ghcr.io/trilitech/jstz-cli:$JSTZ_VERSION'
EOF
            exit 1
    esac

    # Check if the alias already exists to avoid duplicates
    if ! grep -q "alias jstz=" "$shellrc"; then
        # Append the alias to the shell configuration file
        echo "$shell_alias" >> "$shellrc"
        echo "Alias added to $shellrc."

        # shellcheck disable=SC1090 
        # `$shellrc`` can only be determined at runtime, so we need to disable the warning.
        # 
        # Reload the shell configuration file to apply the changes 
        . "$shellrc"
    else
        echo "Alias already exists in $shellrc. Skipping..."
    fi
}

do_install() {
    jstz_download
    jstz_configure
    jstz_reset
}

jstz_reset() {
    unset -f jstz_download jstz_configure jstz_reset do_install
}

do_install

} 