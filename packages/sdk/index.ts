import * as jstz from "jstz_sdk";

namespace ffi {
  export type Address = { Tz1: string };

  export type Signature = { Ed25519: string };

  export type PublicKey = { Ed25519: string };

  export type Operation = {
    source: Address;
    nonce: number;
    content: OperationContent;
  };

  export type Headers = Record<string, string>;
  export type Body = Uint8Array;

  export type OperationContent =
    | { DeployFunction: { function_code: string; account_credit: number } }
    | {
        RunFunction: {
          uri: string;
          method: string;
          headers: Headers;
          body: Body | null;
          gas_limit: number;
        };
      };

  export type SignedOperation = {
    public_key: PublicKey;
    signature: Signature;
    inner: Operation;
  };

  export type Receipt = {
    hash: Uint8Array;
    inner: ReceiptResult;
  };

  export type ReceiptResult = { Ok: ReceiptContent } | { Err: string };

  export type ReceiptContent =
    | {
        RunFunction: {
          body: Body;
          status_code: number;
          headers: Headers;
        };
      }
    | {
        DeployFunction: {
          address: Address;
        };
      };
}

export type Address = string;

const ADDRESS_REGEX = /^tz1[a-zA-Z0-9]{33}$/;

export function isAddress(value: unknown): value is Address {
  return typeof value === "string" && value.match(ADDRESS_REGEX) !== null;
}

interface Operation {
  source: Address;
  nonce: number;
  content: OperationContent;
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

type OperationContent =
  | {
      kind: "deploy";
      functionCode: string;
      initialBalance: number;
    }
  | ({
      kind: "run";
    } & JstzRequest);

export type User = {
  address: Address;
  publicKey: string;
  secretKey: string;
};

type SignedOperation = {
  publicKey: string;
  signature: string;
  hash: string;
  operation: Operation;
};

const encodeAddress = (address: Address): ffi.Address => {
  return { Tz1: address };
};

const encodeSignature = (signature: string): ffi.Signature => {
  return { Ed25519: signature };
};

const encodePublicKey = (publicKey: string): ffi.PublicKey => {
  return { Ed25519: publicKey };
};

const encodeOperationContent = (
  content: OperationContent,
): ffi.OperationContent => {
  switch (content.kind) {
    case "deploy":
      return {
        DeployFunction: {
          function_code: content.functionCode,
          account_credit: content.initialBalance,
        },
      };
    case "run":
      return {
        RunFunction: {
          uri: content.uri,
          method: content.method || "GET",
          headers: content.headers || {},
          body: content.body === undefined ? null : content.body,
          gas_limit: content.gasLimit || 1000,
        },
      };
  }
};

const encodeOperation = (operation: Operation): ffi.Operation => {
  const { source, nonce, content } = operation;

  return {
    source: encodeAddress(source),
    nonce,
    content: encodeOperationContent(content),
  };
};

const encodeSignedOperation = (
  signedOperation: SignedOperation,
): ffi.SignedOperation => {
  const { publicKey, signature, operation } = signedOperation;

  return {
    public_key: encodePublicKey(publicKey),
    signature: encodeSignature(signature),
    inner: encodeOperation(operation),
  };
};

const signOperation = (user: User, operation: Operation): SignedOperation => {
  const ffiOperation = encodeOperation(operation);

  const signature = jstz.sign_operation(ffiOperation, user.secretKey);
  const hash = jstz.hash_operation(ffiOperation);

  return { publicKey: user.publicKey, signature, hash, operation };
};

export class Jstz {
  private endpoint: string;
  constructor(endpoint: string) {
    this.endpoint = endpoint;
  }

  async getNonce(source: Address): Promise<number> {
    const res = await fetch(`http://${this.endpoint}/accounts/${source}/nonce`);

    if (res.status === 404) {
      return 0;
    }

    if (res.status !== 200) {
      console.log(res);
      throw new Error("Failed to fetch nonce");
    }

    return (await res.json()) as number;
  }

  private pollReceipt(hash: string): Promise<ffi.Receipt> {
    const endpoint = this.endpoint;
    return new Promise((resolve, reject) => {
      const interval = setInterval(async () => {
        try {
          const res = await fetch(
            `http://${endpoint}/operations/${hash}/receipt`,
          );
          if (res.status === 200) {
            const receipt = (await res.json()) as ffi.Receipt;
            clearInterval(interval);
            resolve(receipt);
          }
        } catch (err) {
          clearInterval(interval);
          reject(err);
        }
      }, 1000);
    });
  }

  private async postSignedOperation(
    operation: SignedOperation,
  ): Promise<ffi.Receipt> {
    await fetch(`http://${this.endpoint}/operations`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(encodeSignedOperation(operation)),
    });

    const receipt = await this.pollReceipt(operation.hash);

    return receipt;
  }

  async deploy(
    user: User,
    functionCode: string,
    initialBalance: number = 0,
  ): Promise<Address> {
    const nonce = await this.getNonce(user.address);

    const operation: Operation = {
      source: user.address,
      nonce,
      content: {
        kind: "deploy",
        functionCode,
        initialBalance,
      },
    };

    const receipt = await this.postSignedOperation(
      signOperation(user, operation),
    );

    if ("Err" in receipt.inner) {
      throw new Error(receipt.inner.Err);
    }

    const receiptContent = receipt.inner["Ok"];

    if (!("DeployFunction" in receiptContent)) {
      throw new Error("Unexpected receipt kind");
    }

    return receiptContent.DeployFunction.address.Tz1;
  }

  async run(user: User, request: JstzRequest): Promise<JstzResponse> {
    const nonce = await this.getNonce(user.address);

    const operation: Operation = {
      source: user.address,
      nonce,
      content: {
        kind: "run",
        ...request,
      },
    };

    const receipt = await this.postSignedOperation(
      signOperation(user, operation),
    );

    if ("Err" in receipt.inner) {
      throw new Error(receipt.inner.Err);
    }

    const receiptContent = receipt.inner["Ok"];

    if (!("RunFunction" in receiptContent)) {
      throw new Error("Unexpected receipt kind");
    }

    return {
      statusCode: receiptContent.RunFunction.status_code,
      headers: receiptContent.RunFunction.headers,
      body: receiptContent.RunFunction.body,
    };
  }
}
