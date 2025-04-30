# 👨‍⚖️ jstz

[![codecov](https://codecov.io/gh/jstz-dev/jstz/graph/badge.svg?token=FA7IPI5Q9J)](https://codecov.io/gh/jstz-dev/jstz)

`Jstz` (pronounced: "justice") is a JavaScript runtime powered by Tezos Smart Optimistic Rollups that uses [Boa](https://boajs.dev/) and is built in [Rust](https://www.rust-lang.org/).

## Installation

Install the Jstz command-line tool via NPM with this command:

```sh
npm i -g @jstz-dev/cli
```

To verify that Jstz installed correctly, run `jstz --version`.

## Building from source

You can build Jstz from source with Rust:

```sh
make build-deps
make build
```

See [installing Octez](/CONTRIBUTING.md#installing-octez-🐙) for installing the necessary dependencies for running `jstz`.

## Quick Start

```sh
# Make a javascript file
echo "export default (() => new Response('hello world'))" > index.js
# Start the sandbox (as a daemon)
jstz sandbox start -d
# Deploy smart function
jstz deploy index.js --name example
# Send request to smart function
jstz run jstz://example/
```

For a more detailed quick start, see [Quick start](https://jstz-dev.github.io/jstz/quick_start.html).

## Documentation

For the latest `jstz` documentation, [click here](https://jstz-dev.github.io/jstz/).

## Contributing

Please, check the [CONTRIBUTING.md](/CONTRIBUTING.md) file to know how to effectively contribute
to the project.

## License

This project is licensed under the MIT license.
