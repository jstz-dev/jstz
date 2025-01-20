function o(t) {
  return typeof t == "string";
}
function l(t, e) {
  return Array.isArray(e) && e.reduce((r, n) => r && t(n), !0);
}
function d(t) {
  return typeof t == "number" && Number.isInteger(t);
}
function T(t) {
  let e = t;
  try {
    return o(e.to) && d(e.token_id) && Number.isInteger(e.amount);
  } catch {
    return !1;
  }
}
function B(t) {
  let e = t;
  try {
    return o(e.from) && l(T, e.transfers);
  } catch {
    return !1;
  }
}
function $(t) {
  let e = t;
  try {
    return (
      (e.operation === "add_operator" || e.operation === "remove_operator") &&
      o(e.owner) &&
      o(e.operator) &&
      d(e.token_id)
    );
  } catch {
    return !1;
  }
}
function f(t) {
  let e = t;
  try {
    return o(e.owner) && d(e.token_id);
  } catch {
    return !1;
  }
}
function v(t) {
  let e = t;
  try {
    return l(f, e.requests);
  } catch {
    return !1;
  }
}
function N(t) {
  let e = t;
  try {
    return f(e.request) && Number.isInteger(e.balance);
  } catch {
    return !1;
  }
}
function O(t) {
  let e = t;
  try {
    return d(e.token_id) && o(e.owner) && Number.isInteger(e.amount);
  } catch {
    return !1;
  }
}
function y(t) {
  let e = t;
  try {
    return (
      o(e.receiver) &&
      Number.isInteger(e.amount) &&
      typeof e.ticketHash == "string"
    );
  } catch {
    return !1;
  }
}
function g(t, e) {
  return `balance/${t}/${e}`;
}
function k(t, e) {
  return Kv.get(g(t, e)) || 0;
}
function A(t, e, r) {
  if (r < 0) throw "FA2_INSUFFICIENT_BALANCE";
  Kv.set(g(t, e), r);
}
function w(t, e, r) {
  let n = k(t, e);
  A(t, e, n + r);
}
var u = class {
    routes = {};
    addRoute(e, r, n) {
      let { pattern: s, keys: i } = this.pathToRegex(r),
        a = this.routes[e] ?? [];
      a.push({ pattern: s, keys: i, handler: n }), (this.routes[e] = a);
    }
    pathToRegex(e) {
      let r = [],
        n = e.replace(/:([a-zA-Z0-9_]+)/g, (i, a) => (r.push(a), "([^\\/]+)"));
      return (
        (n = n !== e ? n : e.replace(/[-\/\\^$*+?.()|[\]{}]/g, "\\$&")),
        { pattern: new RegExp(`^${n}$`), keys: r }
      );
    }
    async handleRequest(e) {
      let r = new URL(e.url),
        n = e.method,
        s = r.pathname;
      for (let a of this.routes[n] ?? []) {
        let c = s.match(a.pattern);
        if (c) {
          console.log(`Matched: ${n} ${r}`);
          let m = a.keys.reduce((p, R, h) => ((p[R] = c[h + 1]), p), {});
          return await a.handler(e, m);
        }
      }
      let i = `Unrecognised entrypoint ${n} ${s}`;
      return console.error(i), new Response(i, { status: 404 });
    }
  },
  b = () => {
    let t = new u();
    return (
      t.addRoute("GET", "/ping", async (e, r) => new Response("pong")),
      t.addRoute("GET", "/balances/:address", async (e, r) => {
        let n = r.address;
        if (o(n)) {
          let s = k(n, 1),
            i = { request: { owner: n, token_id: 1 }, balance: s };
          return Response.json(i);
        } else return new Response("Not valid address", { status: 400 });
      }),
      t.addRoute("POST", "/-/deposit", async (e, r) => {
        if (e.headers.get("Referer") !== "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx")
          return (
            console.log(`${e.headers.get("Referer")} is not Hosta address`),
            new Response("Referer not set to Host address", { status: 400 })
          );
        let n = await e.json();
        if (y(n)) {
          let s = Kv.get("ticketHash");
          return (
            s ||
              (console.log("setting ticketHash"),
              Kv.set("ticketHash", n.ticketHash),
              (s = n.ticketHash)),
            s !== n.ticketHash
              ? new Response(
                  `Invalid ticketHash. Expected: ${s} Actual: ${n.ticketHash}`,
                  { status: 400 },
                )
              : (console.log(`depositing funds to ${n.receiver}`),
                w(n.receiver, 1, n.amount),
                new Response("Success!"))
          );
        } else
          return (
            console.log("Request is not a bridge deposit"),
            console.log(`Request body: ${JSON.stringify(n)}`),
            Response.error()
          );
      }),
      t
    );
  };
async function x(t) {
  return b().handleRequest(t);
}
var H = x;
export {
  H as default,
  v as isBalanceOf,
  f as isBalanceRequest,
  N as isBalanceResponse,
  y as isBridgeDeposit,
  O as isMintNew,
  d as isTokenId,
  T as isTransfer,
  B as isTransfers,
  $ as isUpdateOperator,
};
