import * as jstz from "jstz_sdk";

import { Jstz as JstzClient } from "@jstz-dev/jstz-client";
import JstzType from "@jstz-dev/jstz-client";

export type Address = string;

const ADDRESS_REGEX = /^tz1[a-zA-Z0-9]{33}$/;

export function isAddress(value: unknown): value is Address {
  return typeof value === "string" && value.match(ADDRESS_REGEX) !== null;
}

export type JstzHeaders = Record<string, string>;
export type JstzBody = Uint8Array;
export type JstzRequest = {
  uri: string;
  method?: string;
  headers?: JstzHeaders;
  body?: JstzBody;
  gasLimit?: number;
};
export type JstzResponse = {
  statusCode: number;
  headers: JstzHeaders;
  body: JstzBody;
};

export type User = {
  address: Address;
  publicKey: string;
  secretKey: string;
};

const signOperation = (
  user: User,
  operation: JstzType.Operation,
): JstzType.Signature => {
  const signature = jstz.sign_operation(operation, user.secretKey);
  return signature;
};

export class Jstz {
  private client: JstzClient;

  constructor(endpoint: string) {
    this.client = new JstzClient({ baseURL: endpoint });
  }

  async getNonce(source: Address): Promise<number> {
    return this.client.accounts.getNonce(source);
  }

  async deploy(
    user: User,
    functionCode: string,
    initialBalance: number = 0,
  ): Promise<Address> {
    const nonce = await this.getNonce(user.address);
    const content: JstzType.Operation.DeployFunction = {
      _type: "DeployFunction",
      functionCode: functionCode,
      accountCredit: initialBalance,
    };

    const operation = {
      publicKey: user.publicKey,
      nonce,
      content,
    };
    const signature = signOperation(user, operation);
    const request = {
      signature,
      inner: operation,
    };
    const receipt = await this.client.operations.injectAndPoll(request);
    return receipt.result.inner.address;
  }

  async run(
    user: User,
    request: JstzRequest,
  ): Promise<JstzType.Receipt.Success.RunFunction> {
    const nonce = await this.getNonce(user.address);
    const content: JstzType.Operation.RunFunction = {
      _type: "RunFunction",
      body: request.body ? Buffer.from(request.body).toString("base64") : null,
      gasLimit: request.gasLimit ?? 1000,
      headers: request.headers ?? {},
      method: request.method ?? "GET",
      uri: request.uri,
    };

    const operation = {
      publicKey: user.publicKey,
      nonce,
      content,
    };

    const signature = signOperation(user, operation);

    const receipt = await this.client.operations.injectAndPoll({
      signature: signature,
      inner: operation,
    });

    return receipt.result.inner;
  }
}
