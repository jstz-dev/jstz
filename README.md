# Jstz

[![codecov](https://codecov.io/gh/jstz-dev/jstz/graph/badge.svg?token=FA7IPI5Q9J)](https://codecov.io/gh/jstz-dev/jstz) [![Nightly build](https://github.com/jstz-dev/jstz/actions/workflows/build-nightly.yml/badge.svg?branch=main)](https://github.com/jstz-dev/jstz/actions/workflows/build-nightly.yml)

Jstz (pronounced: "justice") is a JavaScript runtime powered by Tezos Smart Optimistic Rollups that is built in [Rust](https://www.rust-lang.org/).

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

See [installing Octez](/CONTRIBUTING.md#installing-octez-) for installing the necessary dependencies for running Jstz.

## Quick Start

```sh
# Create a smart function in a JavaScript file
echo "export default (() => new Response('hello world'))" > index.js
# Start the sandbox in detach mode
jstz sandbox --container start -d
# Deploy smart function
jstz deploy index.js --name example -n dev
# Send request to smart function
jstz run jstz://example/ -n dev
```

For a more detailed quick start, see [Quick start](https://jstz.tezos.com/quick_start).

## Documentation

For the latest Jstz documentation, [click here](https://jstz.tezos.com/).

To build the documentation locally with Docusaurus, first do `npm i`, and then:

    npm run docs:dev

to quickly see the results in your browser, or:

    npm run docs:build

to build a production-grade website.

## Contributing

Please, check the [CONTRIBUTING.md](/CONTRIBUTING.md) file to know how to effectively contribute
to the project.

## License

This project is licensed under the MIT license.
