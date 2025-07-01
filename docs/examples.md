---
title: Examples
---

import Image from '@theme/IdealImage';

The Jstz source code repository contains many example smart functions and applications that use them:

https://github.com/jstz-dev/jstz/tree/main/examples

Here are summaries of some of the larger examples:

## Example smart functions

### Hello world

The [`hello-world`](https://github.com/jstz-dev/jstz/blob/main/examples/hello-world/index.ts) smart function accepts a name and returns a hello world message to the account that called it.

### Counter

The [`counter`](https://github.com/jstz-dev/jstz/tree/main/examples/counter) smart function is a simple smart function that stores a number and allows callers to retrieve the number, add one to it, or subtract one to it.
Because nothing aside from the smart function itself can change the number, it demonstrates how a smart function can store persistent data that no other source can manipulate.

For a walkthrough of setting up and deploying this smart function, see the [Quick start](/quick_start).

### get-tez

The [`get-tez`](https://github.com/jstz-dev/jstz/tree/main/examples/get-tez) smart function shows you how smart functions can receive, store, and transfer tez, the primary currency of Jstz and Tezos.
The smart function sends 1 tez to any account that asks, as long as it has enough and the account has not received too many tez.

You can use this example to see how tez works on Jstz and how smart functions work with tez.
Because it keeps track of the messages it receives and the accounts that it has sent tez to, it's also an example of how smart functions [store data](/functions/data_storage).

### Third-party libraries

The [`zod`](https://github.com/jstz-dev/jstz/tree/main/examples/zod) example shows how you can use third-party JavaScript libraries in Jstz smart functions.
It uses [itty-router](https://github.com/kwhitley/itty-router) to route requests and [zod](https://github.com/colinhacks/zod) to validate user input.

### FA2 token

The [`fa2`](https://github.com/jstz-dev/jstz/tree/main/examples/fa2) smart function is an implementation of the Tezos FA2 standard for tokens.
FA2 tokens can be fungible (interchangeable, like other currencies) or non-fungible (unique and not interchangeable).
The smart function allows you to create FA2 tokens, transfer them between accounts, and get the balance of tokens in an owner's account.
In this way, you can create tokens to represent anything that you want them to represent.

For more information about FA2 tokens, see [FA2 tokens](https://docs.tezos.com/architecture/tokens/FA2) on docs.tezos.com.

### URL shortener

The [`url_shortener`](https://github.com/jstz-dev/jstz/tree/main/examples/url_shortener) smart function stores long URLs with a shortcode and returns the full URL when you send the shortcode.

### Other examples

Many other example smart functions are available in the `examples` folder to illustrate different things that you can do with Jstz:

https://github.com/jstz-dev/jstz/tree/main/examples

## Example web applications

### Web application with wallet integration

The example [`web-call-to-jstz`](https://github.com/jstz-dev/dev-wallet/tree/main/examples/web-call-to-jstz) is a web application that signs transactions with the Jstz development wallet.
It requires you to install the dev wallet as described in the repository README: https://github.com/jstz-dev/dev-wallet.
It calls the [Counter](#counter) smart function, but you could change it to call other smart functions.

For a walkthrough of setting up and running this application, see the [Quick start](/quick_start).

<div style={{maxWidth:400}}>
<Image img={require('./static/img/quick_start_web_app.png')} alt="The web application showing the response from a successful call to the sample smart function" width="100"/>
</div>

### Simple web application

The example [`call-from-web`](https://github.com/jstz-dev/jstz/tree/main/examples/call-from-web) is a simple web application that shows how you can call a Jstz smart function from a web application.
It calls the [Counter](#counter) smart function, but you could change it to call other smart functions.

:::warning

This application hard-codes the private key of an account to use to sign transactions to the smart function.
That makes this example appropriate only for development and testing when you don't need to simulate real user interaction.
Do not encode private keys like this.
For an example that uses a wallet, see the [Web application with wallet integration](#web-application-with-wallet-integration).

:::

## Other examples

### show-tez

The [`show-tez`](https://github.com/jstz-dev/jstz/tree/main/examples/show-tez) is a command-line application that is a companion to the `get-tez` example.
It allows users to send and receive requests to a running copy of the `get-tez` smart function.
Users can request tez from the `get-tez` smart function and check the log of the messages that they have sent previously.
