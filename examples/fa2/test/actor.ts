async function handler(request: Request): Promise<Response> {
  const url = new URL(request.url);
  const path = url.pathname;

  try {
    switch (path) {
      case "/ping":
        console.log("Hello from child smart function 👋");
        return new Response("Pong!");

      case "/transfer": {
        let to = url.searchParams.get("to");
        let token_id = +url.searchParams.get("token_id")!;
        let amount = +url.searchParams.get("amount")!;
        let target = url.searchParams.get("fa2");
        let transfers = [
          {
            from: Ledger.selfAddress,
            transfers: [{ to, token_id, amount }],
          },
        ];

        return await SmartFunction.call(
          new Request(`tezos://${target}/transfer`, {
            method: "POST",
            body: JSON.stringify(transfers),
          }),
        );
      }

      case "/add_operator": {
        let target = url.searchParams.get("fa2");
        let tokens = JSON.parse(url.searchParams.get("tokens")!);
        let operator = request.headers.get("Referer");
        let owner = Ledger.selfAddress;

        let body = tokens.map((token_id: number) => ({
          operation: "add_operator",
          owner,
          operator,
          token_id,
        }));

        return await SmartFunction.call(
          new Request(`tezos://${target}/update_operators`, {
            method: "PUT",
            body: JSON.stringify(body),
          }),
        );
      }

      default:
        const error = `Unrecognised entrypoint ${path}`;
        console.error(error);
        return new Response(error, { status: 404 });
    }
  } catch (error) {
    console.error(error);
    return Response.error();
  }
}
export default handler;
