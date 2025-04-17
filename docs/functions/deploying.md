# Deploying smart functions

Deploying a smart function to Jstz is different from deploying most web or JavaScript/TypeScript applications because you cannot change the code of a smart function or delete it after you deploy it.
When you deploy a smart function, Jstz records code of the smart function in its ledger of transactions and there is no way to change or delete entries from the ledger.

## Deploying to the local sandbox

You can use the local sandbox to test smart functions in a simulated environment.

1. Ensure that Docker is installed.

1. Build the smart function as described in [Building smart functions](/functions/building).

1. Start the Jstz sandbox in a Docker container by running `jstz sandbox --container start`.
   The sandbox persists as long as the container is running.

1. Deploy the built smart function with the `jstz deploy` command, including the `-n dev` argument to deploy it to the sandbox, as in this example:

   ```bash
   jstz deploy dist/index.js -n dev
   ```

   If the deployment is successful, the response includes the address of the deployed smart function.

1. If you need to fund the smart function, you can bridge funds from bootstrap accounts with the `jstz bridge deposit` command, as in this example, which uses the variable `<ADDRESS>` for the address of the deployed smart function:

   ```bash
   jstz bridge deposit --from bootstrap1 --to <ADDRESS> --amount 1000 -n dev
   ```

1. Verify that the smart function is deployed by calling it with the `jstz run` command or by checking its balance by running this command:

   ```bash
   jstz account balance -a <ADDRESS> -n dev
   ```

<!-- TODO ## Deploying to Jstz networks -->
