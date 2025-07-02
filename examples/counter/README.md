# Counter smart function

This simple smart function stores a number and allows callers to retrieve the number, add one to it, or subtract one from it.

Follow these steps to use it:

1. Install Jstz, start the sandbox, and create an account as described in the [Quick start](https://jstz.tezos.com/quick_start.html).

2. From the folder with this README.md file, run `npm i` and `npm run build` to build the smart function.

3. Deploy the smart function to the sandbox by running `jstz deploy dist/index.js -n dev`.
   The response includes the address of the deployed smart function.

4. Call the smart function with the Jstz CLI by running this command, replacing `<ADDRESS>` with the address of the deployed smart function:

```shell
jstz run jstz://<ADDRESS>/increment --network dev
```

The response shows that the number in storage was incremented and what the current number is.

For more information about Jstz, see https://jstz.tezos.com/.
