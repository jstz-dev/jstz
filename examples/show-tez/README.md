# Command-line tool

This program is an example of a command-line tool that interacts with Jstz.
It allows users to call the [get-tez function example](https://github.com/jstz-dev/jstz/blob/main/examples/get-tez/README.md) more comfortably than using Jstz command lines, using a dialog involving sending requests, receiving tez, and querying the key-value store.

Follow these steps to use it:

1. Deploy the "get-tez" example as described in its [`README.md`](https://github.com/jstz-dev/jstz/blob/main/examples/get-tez/README.md) file, including bridging some tez to it.

2. Back in the folder of the "show-tez" example (containing the current README.md file), run `npm i` and `npm run build` to build the command-line tool.

3. Make sure that you are logged in to your Jstz account by running `jstz whoami`.
   If you are not logged in, run `jstz login <MY_ACCOUNT>` where `<MY_ACCOUNT` is the local alias of your account.

4. Start the command-line tool by running this command, where the variable `<GET_TEZ>` is the address of the "get-tez" smart function that you deployed:

   ```
   node dist/bundle.js <GET_TEZ>
   ```

   The tool starts and shows a prompt to ask for tez politely.

5. In the prompt, write a message to send to the "get-tez" smart function and press Enter, as in this example:

   ```bash
   Please give me tez.
   ```

   If you passed a polite message (with the word "please" in it), the "get-tez" smart function sends your one XTZ.

6. In the prompt, type `show` and press Enter.
   The tool checks the key-value store and prints the messages that the "get-tez" example has logged.

For more information about Jstz, see https://jstz.tezos.com.
