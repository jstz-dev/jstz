import { Jstz } from "@jstz-dev/jstz-client";
import JstzType from "@jstz-dev/jstz-client";
import { readFileSync } from "fs";

import * as readline from "readline";
import untildify from "untildify";

import * as signer from "jstz_sdk";

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8");

function buildRequest(
  contractAddress: string,
  message: string,
): JstzType.Operation.RunFunction {
  return {
    _type: "RunFunction",
    body: Array.from(
      encoder.encode(
        JSON.stringify({
          message: message,
        }),
      ),
    ),
    gas_limit: 55000,
    headers: {},
    method: "GET",
    uri: `tezos://${contractAddress}`,
  };
}

async function main() {
  const args = process.argv.slice(2);
  const contractAddress = args[0];
  if (!contractAddress) {
    fail("Please provide a smart function address to target");
  }

  const jstzClient = new Jstz({
    timeout: 6000,
  });
  const config = JSON.parse(
    readFileSync(untildify("~/.jstz/config.json"), "utf-8"),
  );

  if (!config.current_alias) {
    fail("User is not logged in. Run `jstz login <alias>` to log in");
  }

  const alias = config.current_alias;

  if (!(config.accounts && config.accounts[alias])) {
    fail(`Could not find user '${alias}' in config`);
  }

  const {
    secret_key: secretKey,
    public_key: publicKey,
    address,
  } = config.accounts[alias].User;
  const terminal = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  });
  let waitingForReceipt = false;
  terminal.on("line", async (input: string) => {
    try {
      if (input.toLocaleLowerCase() === "show") {
        const length: number = Number.parseInt(
          await jstzClient.accounts
            .getKv(contractAddress, {
              key: `messages/${address}/length`,
            })
            .catch(() => {
              console.log("No messages yet.");
              return "0";
            }),
        );
        for (let index = 0; index < length; index++) {
          const message = await jstzClient.accounts.getKv(contractAddress, {
            key: `messages/${address}/${index}`,
          });
          console.log(`[${index}]`, message);
        }
      } else {
        if (waitingForReceipt) {
          return;
        }
        const runFunction = buildRequest(contractAddress, input);
        const nonce = await jstzClient.accounts.getNonce(address);
        const operation = {
          content: runFunction,
          nonce,
          source: address,
        };
        const signature = signer.sign_operation(operation, secretKey);
        const response = jstzClient.operations.injectAndPoll({
          inner: operation,
          public_key: publicKey,
          signature: signature,
        });
        waitingForReceipt = true;
        const {
          result: {
            inner: { body },
          },
        } = await response;
        waitingForReceipt = false;
        if (body) {
          console.log("🤖:", JSON.parse(decoder.decode(new Uint8Array(body))));
        }
      }
    } catch (error) {
      console.log(error);
      waitingForReceipt = false;
    }
  });
}

function fail(message: string) {
  console.log(message);
  process.exit(1);
}

console.log(
  `🤖: Please ask for tez politely. Type "show" to see past messages. Ctrl+C to quit`,
);
(async () => await main())();
