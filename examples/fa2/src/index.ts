import { isAddress } from "@tezos/jstz";

function isArray<T>(check: (item: T) => item is T, list: unknown): list is T[] {
  return (
    Array.isArray(list) && list.reduce((acc, item) => acc && check(item), true)
  );
}

export type TokenId = number;
export function isTokenId(id: unknown): id is TokenId {
  return typeof id === "number" && Number.isInteger(id);
}

export type Transfer = { to: Address; token_id: TokenId; amount: number };
export function isTransfer(argument: unknown): argument is Transfer {
  let transfer = argument as Transfer;
  try {
    return (
      isAddress(transfer.to) &&
      isTokenId(transfer.token_id) &&
      Number.isInteger(transfer.amount)
    );
  } catch {
    return false;
  }
}

export type Transfers = { from: Address; transfers: Transfer[] };
export function isTransfers(argument: unknown): argument is Transfers {
  let transfers = argument as Transfers;
  try {
    return (
      isAddress(transfers.from) && isArray(isTransfer, transfers.transfers)
    );
  } catch {
    return false;
  }
}

export type UpdateOperator = {
  operation: "add_operator" | "remove_operator";
  owner: Address;
  operator: Address;
  token_id: TokenId;
};
export function isUpdateOperator(
  argument: unknown,
): argument is UpdateOperator {
  let update = argument as UpdateOperator;
  try {
    return (
      (update.operation === "add_operator" ||
        update.operation === "remove_operator") &&
      isAddress(update.owner) &&
      isAddress(update.operator) &&
      isTokenId(update.token_id)
    );
  } catch {
    return false;
  }
}

export type BalanceRequest = { owner: Address; token_id: TokenId };
export function isBalanceRequest(
  argument: unknown,
): argument is BalanceRequest {
  let request = argument as BalanceRequest;
  try {
    return isAddress(request.owner) && isTokenId(request.token_id);
  } catch {
    return false;
  }
}
export type BalanceOf = { requests: BalanceRequest[] };
export function isBalanceOf(argument: unknown): argument is BalanceOf {
  let balanceOf = argument as BalanceOf;
  try {
    return isArray(isBalanceRequest, balanceOf.requests);
  } catch {
    return false;
  }
}

export type BalanceResponse = { request: BalanceRequest; balance: number };
export function isBalanceResponse(
  argument: unknown,
): argument is BalanceResponse {
  let balanceResponse = argument as BalanceResponse;
  try {
    return (
      isBalanceRequest(balanceResponse.request) &&
      Number.isInteger(balanceResponse.balance)
    );
  } catch {
    return false;
  }
}

export type MintNew = { token_id: TokenId; owner: Address; amount: number };
export function isMintNew(argument: unknown): argument is MintNew {
  let mintNew = argument as MintNew;
  try {
    return (
      isTokenId(mintNew.token_id) &&
      isAddress(mintNew.owner) &&
      Number.isInteger(mintNew.amount)
    );
  } catch {
    return false;
  }
}

function registerKey(tokenId: TokenId): string {
  return `token/${tokenId}`;
}
function registerToken(tokenId: TokenId): void {
  Kv.set(registerKey(tokenId), true);
}
function assertRegistered(tokenId: TokenId): void {
  if (!Kv.get(registerKey(tokenId))) {
    throw "FA2_TOKEN_UNDEFINED";
  }
}

function balanceKey(user: Address, tokenId: TokenId): string {
  return `balance/${user}/${tokenId}`;
}
function getBalance(user: Address, tokenId: TokenId): number {
  return (Kv.get(balanceKey(user, tokenId)) as number) || 0;
}
function setBalance(user: Address, tokenId: TokenId, newBalance: number) {
  if (newBalance < 0) {
    throw "FA2_INSUFFICIENT_BALANCE";
  }
  Kv.set(balanceKey(user, tokenId), newBalance);
}
function changeBalance(user: Address, tokenId: TokenId, amount: number) {
  const oldBalance = getBalance(user, tokenId);
  setBalance(user, tokenId, oldBalance + amount);
}
function transferTokens(
  from: Address,
  to: Address,
  tokenId: TokenId,
  amount: number,
) {
  changeBalance(from, tokenId, -amount);
  changeBalance(to, tokenId, amount);
}

function operatorKey(
  owner: Address,
  operator: Address,
  token_id: TokenId,
): string {
  return `owner/${owner}/${operator}/${token_id}`;
}
function setOperator(owner: Address, operator: Address, token_id: TokenId) {
  Kv.set(operatorKey(owner, operator, token_id), true);
}
function unsetOperator(owner: Address, operator: Address, token_id: TokenId) {
  Kv.delete(operatorKey(owner, operator, token_id));
}
function assertOperator(owner: Address, operator: Address, token_id: TokenId) {
  if (!(owner === operator || Kv.get(operatorKey(owner, operator, token_id)))) {
    throw "FA2_NOT_OPERATOR";
  }
}
function assertOwner(owner: Address, referer: Address) {
  if (owner !== referer) {
    console.log(`${owner} !== ${referer}`);
    throw "FA2_NOT_OWNER";
  }
}

function performTransfer(from: Address, operator: Address, transfer: Transfer) {
  assertRegistered(transfer.token_id);
  assertOperator(from, operator, transfer.token_id);
  transferTokens(from, transfer.to, transfer.token_id, transfer.amount);
}
function performTransfers(referer: Address, transfers: Transfers[]) {
  transfers.forEach((group) =>
    group.transfers.forEach((transfer) =>
      performTransfer(group.from, referer, transfer),
    ),
  );
}

function performUpdateOperator(referer: Address, update: UpdateOperator) {
  switch (update.operation) {
    case "add_operator":
      assertOwner(update.owner, referer);
      setOperator(update.owner, update.operator, update.token_id);
      break;
    case "remove_operator":
      assertOperator(update.owner, referer, update.token_id);
      unsetOperator(update.owner, update.operator, update.token_id);
  }
}
function performBalanceRequest(request: BalanceRequest): BalanceResponse {
  const balance = getBalance(request.owner, request.token_id);
  return { request, balance };
}
function performBalanceOf(balanceOf: BalanceOf): BalanceResponse[] {
  return balanceOf.requests.map(performBalanceRequest);
}

function performMintNew(mintNew: MintNew) {
  registerToken(mintNew.token_id);
  changeBalance(mintNew.owner, mintNew.token_id, mintNew.amount);
}

async function handler(request: Request): Promise<Response> {
  const url = new URL(request.url);
  const path = url.pathname;

  try {
    switch (path) {
      case "/ping":
        console.log("Hello from runner contract 👋");
        return new Response("Pong");

      case "/balance_of":
        if (request.method === "GET") {
          let balanceOf = {
            requests: JSON.parse(
              TextEncoder.atob(url.searchParams.get("requests") as string),
            ),
          };
          if (isBalanceOf(balanceOf)) {
            let responses = performBalanceOf(balanceOf);
            return Response.json(responses);
          } else {
            console.error("Invalid parameters", balanceOf);
            return Response.error();
          }
        } else {
          const error = "/balance_of is a GET request";
          console.error(error);
          return new Response(error, { status: 500 });
        }

      case "/transfer":
        if (request.method === "POST") {
          let transfers = await request.json();
          if (isArray(isTransfers, transfers)) {
            performTransfers(
              request.headers.get("Referer") as string,
              transfers,
            );
            return new Response("Success!");
          } else {
            console.error("Invalid parameters", JSON.stringify(transfers));
            return Response.error();
          }
        } else {
          const error = "/transfer is a POST request";
          console.error(error);
          return new Response(error, { status: 500 });
        }

      case "/mint_new":
        if (request.method === "POST") {
          let mint = await request.json();
          if (isArray(isMintNew, mint)) {
            // TODO not anybody should be allowed to do this
            mint.forEach(performMintNew);
            return new Response("Success!");
          } else {
            console.error("Invalid parameters", JSON.stringify(mint));
            return Response.error();
          }
        } else {
          const error = "/mint_new is a POST request";
          console.error(error);
          return new Response(error, { status: 500 });
        }

      case "/update_operators":
        if (request.method === "PUT") {
          let updates = await request.json();
          if (isArray(isUpdateOperator, updates)) {
            updates.forEach((update: UpdateOperator) =>
              performUpdateOperator(
                request.headers.get("Referer") as string,
                update,
              ),
            );
            return new Response("Success!");
          } else {
            console.error("Invalid parameters", JSON.stringify(updates));
            return Response.error();
          }
        } else {
          const error = "/update_operators is a PUT request";
          console.error(error);
          return new Response(error, { status: 500 });
        }

      default:
        const error = `Unrecognised entrypoint ${path}`;
        console.error(error);
        return new Response(error, { status: 404 });
    }
  } catch (error) {
    console.error(error);
    throw error;
  }
}
export default handler;
