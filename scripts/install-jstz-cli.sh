#!/usr/bin/env bash

{ # this ensures the entire script is downloaded

  version="20240703"
  network="ghostnet"
  # FIXME: https://app.asana.com/0/1205770721173533/1207698416028745/f
  # Update the container URL to point to jstz-dev instead of trilitech
  # (once the runners have been transferred)
  container="ghcr.io/trilitech/jstz-cli:$version"
  jstz_home="$HOME/.jstz"

  # ps -p filters on the parent process, -o comm= prints the command name (supressing the header)
  current_shell=$(basename "$(ps -p $$ -o comm=)")
  shell=$(basename "$SHELL")

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
    if ! command -v docker &>/dev/null; then
      echo "Docker is not installed. Please install Docker and try again."
      return 1
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

    cat >"$temp_file" <<EOF
{
  "default_network": "dev",
  "networks": {
    "weeklynet": {
      "octez_node_rpc_endpoint": "http://rpc.$network.teztnets.com",
      "jstz_node_endpoint": "http://34.147.156.46:8933"
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

    echo 'Configuring `jstz` alias...'

    echo "SHELL: $shell"
    echo "Detected shell: $current_shell"

    if [[ $shell == "bash" ]]; then
      shellrc="$HOME/.bashrc"
    elif [[ $shell == "zsh" ]]; then
      # Respect the ZDOTDIR variable if set, defaulting to $HOME if not
      shellrc="${ZDOTDIR:-$HOME}/.zshrc"
    else
      echo >&2 "Unsupported shell: $shell. Please manually add the following alias to your shell's configuration file."
      echo >&2 "    $shell_alias"
      return 1
    fi

    if ! [ -w "$shellrc" ]; then
      echo "Warning: $shellrc is not writable. Please manually add the following alias to your shell's configuration file:"
      echo "$shell_alias"

      echo "Once you have added the alias, run the following command to reload the configuration file in your default shell:"
      echo "    source $shellrc"
      return 1
    fi

    if grep -q "alias jstz=" "$shellrc"; then
      sed -i'' -e "/alias jstz=/c\\
$shell_alias" "$shellrc"
      echo "Alias updated in $shellrc."
    else
      echo "$shell_alias" >>"$shellrc"
      echo "Alias added to $shellrc."
    fi
    echo "$shell_alias"

    # shellcheck disable=SC1090
    # `$shellrc` can only be determined at runtime, so we need to disable the warning.
    #
    # Reload the shell configuration file to apply the changes
    if [[ $shell == "$current_shell" ]]; then
      echo "Reloading shell configuration file: $shellrc"
      . "$shellrc"
    else
      echo "The current shell session does not match the default shell. Configuration file not reloaded."
      echo "Please run the following command from your default shell to reload the configuration file:"
      echo "    source $shellrc"
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
