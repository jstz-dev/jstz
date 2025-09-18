async function handleBenchmarkTransaction1(
  request: Request,
): Promise<Response> {
  if (Kv.get("account0") === "0") {
    Kv.set("account0", Kv.get("account1"));
    Kv.set("account1", "0");
  } else if (Kv.get("account1") === "0") {
    Kv.set("account1", Kv.get("account0"));
    Kv.set("account0", "0");
  } else {
    throw new Error("Invalid account state");
  }
  return new Response();
}

async function handleBenchmarkTransaction2(
  request: Request,
): Promise<Response> {
  Kv.set("last_sent", Kv.get("last_sent") + 1);
  // TODO: Update once Ledger is fully replaced in v2 runtime and we can deposit from bootstrap accounts in riscv-sandbox
  if (Kv.get("account1") > 0) {
    Kv.set("account1", Kv.get("account1") - 1);
  } else {
    throw new Error("Account 1 has no funds");
  }

  let receiver = "account" + Kv.get("last_sent");
  if (receiver === "account0") {
    if (Kv.get(receiver) === undefined) {
      Kv.set(receiver, 1);
    } else {
      Kv.set(receiver, Kv.get(receiver) + 1);
    }
    return new Response();
  }
}

async function handleBenchmarkTransaction3(
  request: Request,
): Promise<Response> {
  Kv.set("value0", "47");
  for (let i = 0; i < 5000; i++) {
    Kv.set(`value${i}`, Kv.get(`value${i - 1}`) + 1);
  }
  return new Response();
}

async function handleBenchmarkTransaction4(
  request: Request,
): Promise<Response> {
  const smartFunctionAddress = Kv.get("smartFunctionAddress");
  let response = new Response();
  for (let i = 0; i < 200; i++) {
    response = await SmartFunction.call(
      new Request(`jstz://${smartFunctionAddress}/`, {
        method: "POST",
        body: JSON.stringify({ message: "hello" }),
      }),
    );
  }
  return response;
}

async function handler(request: Request): Promise<Response> {
  const url = new URL(request.url);
  const path = url.pathname;
  switch (path) {
    case "/init":
      Kv.set("account0", "0");
      Kv.set("account1", "470");

      Kv.set("last_sent", "0");
      return new Response("Success!");

    case "/init_4":
      // Similar function to handleBenchmarkTransaction1. Doesn't work currently because of issues with SmartFunction.create
      const newSmartFunctionAddress = await SmartFunction.create(
        `
        async function handler(r){
        if (Kv.get("account0") === undefined) {
          Kv.set("account0", "0");
        }
        if (Kv.get("account1") === undefined) {
          Kv.set("account1", "470");
        }
        if (Kv.get("account0") === "0") {
          Kv.set("account0", Kv.get("account1"));
          Kv.set("account1", "0");
        } else if (Kv.get("account1") === "0") {
          Kv.set("account1", Kv.get("account0"));
          Kv.set("account0", "0");
        } else {
          throw new Error("Invalid account state");
        } return new Response()
        }
        export{ handler as default};
      }`,
      );
      Kv.set("smartFunctionAddress", newSmartFunctionAddress);
      return new Response("Success!");

    case "/benchmark_transaction1":
      return handleBenchmarkTransaction1(request);

    case "/benchmark_transaction2":
      return handleBenchmarkTransaction2(request);

    case "/benchmark_transaction3":
      return handleBenchmarkTransaction3(request);

    case "/benchmark_transaction4":
      return handleBenchmarkTransaction4(request);

    case "/check":
      console.log("Checking...");
      // Checking logic can be implemented here.
      console.log("Checks succeeded.");
      return new Response("Success!");
  }
  return new Response("Unrecognized entrypoint", { status: 404 });
}
export default handler;
