async function handleBenchmarkTransaction1(
  request: Request,
): Promise<Response> {
  Kv.set("value", "47");
  return new Response();
}

async function handleBenchmarkTransaction2(
  request: Request,
): Promise<Response> {
  for (let i = 0; i < 1000; i++) {
    Kv.set("value", i);
  }
  return new Response();
}

async function handleBenchmarkTransaction3(
  request: Request,
): Promise<Response> {
  Kv.set("value0", "47");
  for (let i = 0; i < 1000; i++) {
    Kv.set(`value${i}`, Kv.get(`value${i - 1}`) + 1);
  }
  return new Response();
}

async function handler(request: Request): Promise<Response> {
  const url = new URL(request.url);
  const path = url.pathname;
  switch (path) {
    case "/init":
      return new Response("Success!");

    case "/benchmark_transaction1":
      return handleBenchmarkTransaction1(request);

    case "/benchmark_transaction2":
      return handleBenchmarkTransaction2(request);

    case "/benchmark_transaction3":
      return handleBenchmarkTransaction3(request);

    case "/check":
      console.log("Checking...");
      // Checking logic can be implemented here.
      console.log("Checks succeeded.");
      return new Response("Success!");
  }
  return new Response("Unrecognized entrypoint", { status: 404 });
}
export default handler;
