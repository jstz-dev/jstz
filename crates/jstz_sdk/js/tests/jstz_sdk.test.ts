import { describe, it, expect } from "vitest";
import {
  sign_operation,
  hash_operation,
  convert_passkey_signature,
} from "../../pkg/jstz_sdk.js";

const operation = {
  content: {
    _type: "DeployFunction",
    functionCode:
      'export default async () => {\n    console.log("function");\n};\n',
    accountCredit: 0,
  },
  nonce: 0,
  publicKey: "edpkurYYUEb4yixA3oxKdvstG8H86SpKKUGmadHS6Ju2mM1Mz1w5or",
};

describe("Convert passkey signature", () => {
  it("converts to tezos signature", () => {
    const signature = convert_passkey_signature(
      "MEUCIQDv38zGXtPOEc3vO0SVloXyH2ipxd2ACyyDr1HlwrRCHgIgeYcrdOvoPm8nY_jhjtKbqJwVNrGYaf6Yv0l0EKAmNNk",
    );
    expect(
      signature,
      "p2sigtghDmmBqGocWksbS78H4GeEjcahkYMabd5on2Sur9vMbJ1oTwAdpmTTVq4tJhLPLbiPvkb3N821bp7UZ7szjcJLF46uZJ",
    );
  });

  it("fails if input is not a passkey signature", () => {
    expect(() =>
      convert_passkey_signature("Not passkey signature"),
    ).toThrowError(
      "PasskeyError: Base64DecodeError: Encoded text cannot have a 6-bit remainder.",
    );
  });
});

describe("Sign operation", () => {
  const secretKey = "edsk38mmuJeEfSYGiwLE1qHr16BPYKMT5Gg1mULT7dNUtg3ti4De3a";

  it("signs Jstz operations", () => {
    expect(sign_operation(operation, secretKey)).toEqual(
      "edsigtwqy6s8i5ezpHgYnGoDHRkTKf3aQ211WxXrLJJJ7jYYxu6Xpen9BiG6ymRG64zaQFm2tFrff8EuzwD7nfCByMZhr6Nn6CG",
    );
  });

  it("fails to sign objects that are not valid jstz operation", () => {
    let badOperation = {
      content: {},
      nonce: 0,
      publicKey: "edpkurYYUEb4yixA3oxKdvstG8H86SpKKUGmadHS6Ju2mM1Mz1w5or",
    };
    expect(() => sign_operation(badOperation, secretKey)).toThrowError();
  });

  it("fails to sign using unsupported secret keys", () => {
    // BLS keys are not supported
    let badOperation = Object.assign({}, operation);
    badOperation.publicKey =
      "BLpk1tjCNiRsMFAnyLVVyPnNfRitoBHgaHQQnyri6Y2UoTUm4EZgiRmUZXYNjcsFELCEw6ZtiW34";

    expect(() =>
      sign_operation(
        operation,
        "BLsk2aQhPAH9qP2dzVxGeLCnqMfHcWDMyAQ3jupsQQvngjbgppyrov",
      ),
    ).toThrowError("InvalidSecretKey");
  });
});

describe("Hash operation", () => {
  it("hashes Jstz operation", () => {
    let hash = hash_operation(operation);
    expect(hash).toEqual(
      "fdb8f01beba983c723a7c6c28462fe11d3786ecf261d2fdb29a2ec313e802262",
    );
  });

  it("fails to hash objects that are not valid jstz operations", () => {
    let badOperation = "abc123";

    expect(() => hash_operation(badOperation)).toThrowError();
  });
});
