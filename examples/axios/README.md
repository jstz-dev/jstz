# Axios HTTP client

This smart function shows how one can use Axios as an HTTP client for querying both http endpoints and jstz smart functions

Follow these steps to use it

1.  Set up the Jstz local sandbox

    1. [Install Jstz](https://jstz.tezos.com/installation)
    1. Start the [sandbox](https://jstz.tezos.com/sandbox)
    1. Create a Jstz [account](https://jstz.tezos.com/architecture/accounts)

2.  From the folder with this README.md file, run `npm i` and `npm run build` to build the smart function

3.  Deploy the smart function to the sandbox by running `npm run deploy`. This will deploy 2 smart functions;

    - `echo.js` accepts a post request and simply returns request's body in the response.

    - `index.ts` makes a request to `jstz://<echo sf adddress>` and `http://httpbin.org/uuid` using the axios client. The bodies are logged and the smart function simply returns `OK!`

4.  Call the smart function with the Jstz CLI by running this command, replacing `<ECHO ADDRESS>` with the address of the echo smart function address

```shell
jstz run jstz://axios.example -m POST -d '{ "echoSf": "<ECHO ADDRESS>" }' -t --network dev
```

For more information about Jstz, see https://jstz.tezos.com/.
