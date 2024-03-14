#!/usr/bin/env bash

{ # this ensures the entire script is downloaded

version="20240313"
network="weeklynet-2024-03-13"
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
    echo -n "Configuring jstz..."

    mkdir -p "$jstz_home"
    
    config_file="$jstz_home/config.json"
    temp_file=$(mktemp)

    cat >"$temp_file" << EOF
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

    if [ -f "$config_file" ]; then
        if ! cmp -s "$config_file" "$temp_file"; then
            echo "Configuration differs. Creating a backup..."
            backup_file="${config_file}.bak"
            cp "$config_file" "$backup_file"
            echo "Backup created at $backup_file"

            cp "$temp_file" "$config_file"
            echo "Configuration updated."
        else
            echo "Configuration unchanged. No update needed."
        fi
    else
        mv "$temp_file" "$config_file"
        echo "Configuration created."
    fi

    # Cleanup the temporary file if it still exists
    [ -f "$temp_file" ] && rm "$temp_file"

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
    $shell_alias
EOF
            exit 1
    esac

    if ! [ -w "$shellrc" ]; then
        echo "Warning: $shellrc is not writable. Please manually add the following alias to your shell's configuration file:"
        echo "$shell_alias"

        echo "Once you have added the alias, run the following command to reload the configuration file in your default shell:"
        echo "    source $shellrc"
    fi

    if grep -q "alias jstz=" "$shellrc"; then
        sed -i'' -e "/alias jstz=/c\\
$shell_alias" "$shellrc"
        echo "Alias updated in $shellrc."
    else
        echo "$shell_alias" >> "$shellrc"
        echo "Alias added to $shellrc."
    fi

    # shellcheck disable=SC1090
    # `$shellrc` can only be determined at runtime, so we need to disable the warning.
    # 
    # Reload the shell configuration file to apply the changes 
    . "$shellrc"
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