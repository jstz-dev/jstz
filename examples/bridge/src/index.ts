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

export type WithdrawRequest = { amount: number; receiver: Address };
export function isWithdrawRequest(
  argument: unknown,
): argument is WithdrawRequest {
  let withdrawRequest = argument as WithdrawRequest;
  try {
    return (
      isAddress(withdrawRequest.receiver) &&
      Number.isInteger(withdrawRequest.amount)
    );
  } catch {
    return false;
  }
}

export type BridgeInfo = {
  ticket_info: {
    id: number;
    content: number[];
    ticketer: string;
  };
  proxy_l1_contract: string;
};

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

export type BridgeDeposit = {
  receiver: {
    Tz1: Address;
  };
  amount: number;
  ticketHash: string;
};
export function isBridgeDeposit(argument: unknown): argument is BridgeDeposit {
  let bridgeDeposit = argument as BridgeDeposit;
  try {
    return (
      isAddress(bridgeDeposit.receiver.Tz1) &&
      Number.isInteger(bridgeDeposit.amount) &&
      typeof bridgeDeposit.ticketHash === "string"
    );
  } catch {
    return false;
  }
}

export type AddTicketHash = { ticketHash: string; tokenId: number };

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
    console.log("Hello from runner smart function 👋");
    return new Response("pong");
  });

  router.addRoute("GET", "/account/balance", async (request, params) => {
    const owner = request.headers.get("Referer") as string;
    const balance = getBalance(owner, 1);
    const response: BalanceResponse = {
      request: {
        owner,
        token_id: 1,
      },
      balance,
    };
    return Response.json(response);
  });

  router.addRoute("POST", "/-/deposit", async (request, params) => {
    // Check if Referer is null address
    if (
      (request.headers.get("Referer") as string) !==
      "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx"
    ) {
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
      changeBalance(bridgeDeposit.receiver.Tz1, 1, bridgeDeposit.amount);
      return new Response("Success!");
    } else {
      return Response.error();
    }
  });

  router.addRoute("POST", "/bridge/set", async (request, params) => {
    // TODO: Validate
    // TODO: BridgeInfo should contain ticket hash
    let bridgeInfo = (await request.json()) as BridgeInfo;
    Kv.set("bridge-info", bridgeInfo);
    return new Response(`Bridge updated:\n${JSON.stringify(bridgeInfo)}`);
  });

  router.addRoute("POST", "/account/withdraw", async (request, params) => {
    const withdrawRequest = await request.json();
    if (isWithdrawRequest(withdrawRequest)) {
      const owner = request.headers.get("Referer") as string;

      // throws if new balance is < 0
      changeBalance(owner, 1, -withdrawRequest.amount);
      const bridge = Kv.get("bridge-info") as BridgeInfo;
      const body = JSON.stringify({
        amount: withdrawRequest.amount,
        routing_info: {
          receiver: {
            Tz1: withdrawRequest.receiver,
          },
          proxy_l1_contract: bridge.proxy_l1_contract,
        },
        ticket_info: bridge.ticket_info,
      });
      let ticketWithdrawRequest = new Request("tezos://jstz/fa-withdraw", {
        method: "POST",
        headers: {
          "Content-type": "application/json",
        },
        body,
      });
      return await SmartFunction.call(ticketWithdrawRequest);
    } else {
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
