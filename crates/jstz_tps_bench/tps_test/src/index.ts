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
  Kv.set("last_sent", (parseInt(Kv.get("last_sent")) + 1).toString());
  // TODO: Update once Ledger is fully replaced in v2 runtime and we can deposit from bootstrap accounts in riscv-sandbox
  if (Kv.get("account1") > 0) {
    Kv.set("account1", (parseInt(Kv.get("account1")) - 1).toString());
  } else {
    throw new Error("Account 1 has no funds");
  }

  let receiver = "account" + Kv.get("last_sent");
  if (Kv.get(receiver) === undefined) {
    Kv.set(receiver, 1);
  } else {
    Kv.set(receiver, (parseInt(Kv.get(receiver)) + 1).toString());
  }
  return new Response();
}

async function handleBenchmarkTransaction3(
  request: Request,
): Promise<Response> {
  for (let i = 1; i < 5000; i++) {
    Kv.set(`value${i}`, (parseInt(Kv.get(`value${i - 1}`)) + 1).toString());
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

function assert(condition) {
  if (!condition) {
    throw "Assertion failed";
  }
}

async function handler(request: Request): Promise<Response> {
  const url = new URL(request.url);
  const path = url.pathname;
  switch (path) {
    case "/init_1":
      Kv.set("account0", "0");
      Kv.set("account1", "470");
      return new Response("Success!");

    case "/init_2":
      Kv.set("last_sent", "0");
      Kv.set("account1", "470");
      return new Response("Success!");

    case "/init_3":
      Kv.set("value0", "47");
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

    case "/check_1":
      console.log("Checking...");
      assert(Kv.get("account0") === "0");
      assert(Kv.get("account1") === "470");
      console.log("Checks succeeded.");
      return new Response("Success!");

    case "/check_2":
      console.log("Checking...");
      assert(parseInt(Kv.get("last_sent")) > 0);
      assert(parseInt(Kv.get("account1")) < 470);
      console.log("Checks succeeded.");
      return new Response("Success!");

    case "/check_3":
      console.log("Checking...");
      assert(parseInt(Kv.get("value4999")) > 0);
      console.log("Checks succeeded.");
      return new Response("Success!");

    case "/check_4":
      console.log("Checking...");
      // TODO: Add checks for nested sf
      assert(false);
      console.log("Checks succeeded.");
      return new Response("Success!");
  }
  return new Response("Unrecognized entrypoint", { status: 404 });
}
export default handler;
