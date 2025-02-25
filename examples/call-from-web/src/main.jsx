import "./style.css";
import React, { useState } from "react";
import { createRoot } from "react-dom/client";

import { Jstz } from "@jstz-dev/jstz-client";
import * as signer from "jstz_sdk";
const decoder = new TextDecoder("utf-8");

function buildRequest(contractAddress, path) {
  return {
    _type: "RunFunction",
    gas_limit: 55000,
    headers: {},
    method: "GET",
    uri: `tezos://${contractAddress}${path}`,
  };
}

const App = () => {
  const [smartFunctionAddress, setSmartFunctionAddress] = useState("");
  const [accountAddress, setAccountAddress] = useState("");
  const [publicKey, setPublicKey] = useState("");
  const [secretKey, setSecretKey] = useState("");
  const [notificationText, setNotificationText] = useState(
    "Enter your information to call a smart function.",
  );

  const callSmartFunction = async (pathToCall) => {
    setNotificationText("Calling the smart function...");

    const jstzClient = new Jstz({
      timeout: 6000,
    });

    const runFunctionRequest = buildRequest(smartFunctionAddress, pathToCall);

    const nonce = await jstzClient.accounts
      .getNonce(accountAddress)
      .catch(console.error);
    if (nonce === undefined) {
      setNotificationText(
        "This account has not been revealed; make an XTZ transaction with the account before calling a smart function.",
      );
      return;
    }

    try {
      const operation = {
        content: runFunctionRequest,
        nonce,
        source: accountAddress,
      };
      // Sign operation using provided secret key
      // DO NOT use this in production until Jstz has a way of signing in a secure manner
      const signature = signer.sign_operation(operation, secretKey);
      const response = await jstzClient.operations.injectAndPoll({
        inner: operation,
        public_key: publicKey,
        signature: signature,
      });
      const {
        result: {
          inner: { body },
        },
      } = await response;

      const returnedMessage = body
        ? JSON.parse(decoder.decode(new Uint8Array(body)))
        : "No message.";
      setNotificationText("Completed call. Response: " + returnedMessage);
    } catch (err) {
      setNotificationText(err);
    }
  };

  return (
    <div>
      <h1>Call the counter smart function</h1>
      <div className="fields">
        <div>
          <label>
            <a
              href="https://github.com/jstz-dev/jstz/blob/main/examples/counter/README.md"
              target="_blank"
            >
              Counter
            </a>{" "}
            smart function address:
          </label>
          <input
            type="text"
            value={smartFunctionAddress}
            onChange={(e) => setSmartFunctionAddress(e.target.value)}
          ></input>
        </div>
        <div>
          <label>Jstz account address:</label>
          <input
            type="text"
            value={accountAddress}
            onChange={(e) => setAccountAddress(e.target.value)}
          ></input>
        </div>
        <div>
          <label>Public key:</label>
          <input
            type="text"
            value={publicKey}
            onChange={(e) => setPublicKey(e.target.value)}
          ></input>
        </div>
        <div>
          <label>Secret key:</label>
          <input
            type="text"
            value={secretKey}
            onChange={(e) => setSecretKey(e.target.value)}
          ></input>
        </div>
      </div>
      <div className="buttons">
        <button onClick={() => callSmartFunction("/get")}>Get</button>
        <button onClick={() => callSmartFunction("/increment")}>
          <code>Increment</code>
        </button>
        <button onClick={() => callSmartFunction("/decrement")}>
          <code>Decrement</code>
        </button>
      </div>
      <div className="notificationText">{notificationText}</div>
      <div className="signingWarning">
        WARNING: This application does not encrypt private keys and therefore
        should not be used in production. This application is a demonstration of
        how Jstz works and not a secure application.
      </div>
    </div>
  );
};

const root = createRoot(document.getElementById("root"));

root.render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
