import { isAddress } from "@jstz-dev/jstz";

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

export type BridgeDeposit = {
  receiver: Address;
  amount: number;
  ticketHash: string;
};
export function isBridgeDeposit(argument: unknown): argument is BridgeDeposit {
  let bridgeDeposit = argument as BridgeDeposit;
  try {
    return (
      isAddress(bridgeDeposit.receiver) &&
      Number.isInteger(bridgeDeposit.amount) &&
      typeof bridgeDeposit.ticketHash === "string"
    );
  } catch {
    return false;
  }
}

export type AddTicketHash = { ticketHash: string; tokenId: number };

function registerKey(tokenId: TokenId): string {
  return `token/${tokenId}`;
}
function registerToken(tokenId: TokenId): void {
  const key = registerKey(tokenId);
  if (Kv.get(key)) {
    throw "FA2_TOKEN_ID_EXISTS";
  }
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

function performBalanceRequest(request: BalanceRequest): BalanceResponse {
  const balance = getBalance(request.owner, request.token_id);
  console.log(`${request.owner} has ${balance} of token ${request.token_id}`);
  return { request, balance };
}

type RouteHandler = (
  request: Request,
  params: { [key: string]: string | undefined },
) => Promise<Response>;

class Router {
  private routes: {
    // method => handler
    [method: string]: {
      pattern: RegExp;
      keys: string[];
      handler: RouteHandler;
    }[];
  } = {};

  addRoute(method: string, path: string, handler: RouteHandler): void {
    const { pattern, keys } = this.pathToRegex(path);
    let handlers = this.routes[method] ?? [];
    handlers.push({ pattern, keys, handler });
    this.routes[method] = handlers;
  }

  private pathToRegex(path: string): { pattern: RegExp; keys: string[] } {
    const keys: string[] = [];
    let pattern = path.replace(/:([a-zA-Z0-9_]+)/g, (_, key) => {
      keys.push(key);
      return "([^\\/]+)"; // Match dynamic parameters
    });
    pattern =
      pattern !== path
        ? pattern
        : path.replace(/[-\/\\^$*+?.()|[\]{}]/g, "\\$&");
    const regexPattern = new RegExp(`^${pattern}$`);
    return { pattern: regexPattern, keys };
  }

  async handleRequest(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const method = request.method;
    const path = url.pathname;
    for (const route of this.routes[method] ?? []) {
      const match = path.match(route.pattern);
      if (match) {
        console.log(`Matched: ${method} ${url}`);
        const params = route.keys.reduce(
          (acc, key, index) => {
            acc[key] = match[index + 1];
            return acc;
          },
          {} as { [key: string]: string | undefined },
        );
        return await route.handler(request, params);
      }
    }
    const error = `Unrecognised entrypoint ${method} ${path}`;
    console.error(error);
    return new Response(error, { status: 404 });
  }
}

const getRouter = () => {
  const router = new Router();

  router.addRoute("GET", "/ping", async (request, param) => {
    return new Response("pong");
  });

  router.addRoute("GET", "/balances/:address", async (request, params) => {
    const owner = params.address as Address;
    if (isAddress(owner)) {
      const balance = getBalance(owner, 1);
      const response: BalanceResponse = {
        request: {
          owner,
          token_id: 1,
        },
        balance,
      };
      return Response.json(response);
    } else {
      return new Response("Not valid address", { status: 400 });
    }
  });

  router.addRoute("POST", "/-/deposit", async (request, params) => {
    // Check if Referer is null address
    if (
      (request.headers.get("Referer") as string) !==
      "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
    ) {
      console.log(`${request.headers.get("Referer")} is not Hosta address`);
      return new Response("Referer not set to Host address", {
        status: 400,
      });
    }
    // receiver, amount, ticketHash
    let bridgeDeposit = await request.json();
    if (isBridgeDeposit(bridgeDeposit)) {
      let ticketHash = Kv.get("ticketHash") as string | null;
      if (!ticketHash) {
        console.log("setting ticketHash");
        Kv.set("ticketHash", bridgeDeposit.ticketHash);
        ticketHash = bridgeDeposit.ticketHash;
      }
      if (ticketHash !== bridgeDeposit.ticketHash) {
        return new Response(
          `Invalid ticketHash. Expected: ${ticketHash} Actual: ${bridgeDeposit.ticketHash}`,
          { status: 400 },
        );
      }
      console.log(`depositing funds to ${bridgeDeposit.receiver}`);
      changeBalance(bridgeDeposit.receiver, 1, bridgeDeposit.amount);
      return new Response("Success!");
    } else {
      console.log("Request is not a bridge deposit");
      console.log(`Request body: ${JSON.stringify(bridgeDeposit)}`);
      return Response.error();
    }
  });

  return router;
};

async function handler(request: Request): Promise<Response> {
  const router = getRouter();
  return router.handleRequest(request);
}

export default handler;
