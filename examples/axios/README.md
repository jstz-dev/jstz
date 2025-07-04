# Axios HTTP client

This smart function shows how smart functions can use Axios as an HTTP client for querying both HTTP endpoints and Jstz smart functions.

It includes two smart functions:

- `echo.js`: A simple smart function that accepts a text string via a POST request and returns it, to simulate a simple asynchronous smart function

- `index.ts`: A smart function that makes two calls with Axios: a call to the `echo.js` smart function and a call to the external API `http://httpbin.org/uuid` via the enshrined oracle

Follow these steps to deploy and use these smart functions:

1. Set up the Jstz local sandbox:

   1. [Install Jstz](https://jstz.tezos.com/installation).
   1. Start the [sandbox](https://jstz.tezos.com/sandbox).
   1. Create a Jstz [account](https://jstz.tezos.com/architecture/accounts).

2. From the folder with this README.md file, run `npm i` and `npm run build` to build the smart functions.

3. Deploy the smart function to the sandbox by running `npm run deploy`. This script deploys both smart functions and prints their addresses and assigns the local alias `axios.example` to the `index.ts` smart function.

4. Call the `index.ts` smart function with the Jstz CLI by running this command, replacing the placeholder `<ECHO_ADDRESS>` with the address of the echo smart function:

```shell
jstz run jstz://axios.example -m POST -d '{ "echoSf": "<ECHO_ADDRESS>" }' -t --network dev
```

The smart function logs the responses from the external API and the echo smart function, as in this example response:

```
Connected to trace smart function "KT1W6To7ubmoWLQPojrJKKsabpoqsk85Jqhw"
[INFO]: UUID 72e20178-5a4b-49c2-b2ec-e1b75d298c2f

[INFO]: { body: "Hello world!" }

OK!
```

For more information about calling APIs with smart functions, including why you must call them before reading from or writing to the key-value store, see [Enshrined oracle](https://jstz.tezos.com/architecture/oracle) in the Jstz documentation.

For more information about Jstz, see https://jstz.tezos.com/.
