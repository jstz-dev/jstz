# Get-tez smart function

This example smart function stores tez and sends 1 tez (the primary cryptocurrency token of the Tezos chain and therefore the primary token of Jstz) to requesters who ask politely.

Follow these steps to use it:

1. Set up the Jstz local sandbox:

   1. [Install Jstz](https://jstz.tezos.com/installation).
   1. Start the [sandbox](https://jstz.tezos.com/sandbox).
   1. Create a Jstz [account](https://jstz.tezos.com/architecture/accounts).

2. Build the example by going to the folder with this README.md file in a terminal window and running these commands:

   ```bash
   npm i; npm run build
   ```

3. Deploy the smart function by running this command:

   ```bash
   jstz deploy dist/index.js -n dev
   ```

   The response includes the address of the newly deployed smart function.

4. Bridge some tez to the smart function by running this command, where the variable `<GET_TEZ>` is the address:

   ```
   jstz bridge deposit --from bootstrap1 --to <GET_TEZ> --amount 1000 -n dev
   ```

5. Ask the smart function for tez in an impolite way by running this command:

   ```sh
   jstz run jstz://<GET_TEZ>/ --data '{"message":"Give me tez now."}' -n dev
   ```

   The smart function returns the message "Sorry, I only fulfill polite requests."

6. Ask the smart function politely by running this command, which includes the word "please" in the message:

   ```sh
   jstz run jstz://<GET_TEZ>/ --data '{"message":"Please, give me some tez."}' -n dev
   ```

   The function returns the message "Thank you for your polite request. You received 1 tez!"

7. Check your balance by running this command:

   ```sh
   jstz account balance -n dev
   ```

   The response is the current balance of the currently logged in account, including the 1 tez that the smart function sent.

To continue working with this smart function, see the [Command-line tool](https://github.com/jstz-dev/jstz/blob/main/examples/show-tez/README.md) example.
