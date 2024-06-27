async function fa2_balance_of(fa2, minter, token_id) {
  const balance_request = [{ owner: minter, token_id }];

  const encodedRequests = btoa(JSON.stringify(balance_request));

  const response = await fetch(
    new Request(`tezos://${fa2}/balance_of?requests=${encodedRequests}`),
  );
  const balances = await response.json();
  console.log(
    `Address ${balances[0].request.owner} has ${balances[0].balance}`,
  );
}

async function handler(request) {
  const url = new URL(request.url);
  const n = Number(url.searchParams.get("n"));
  const fa2 = url.searchParams.get("fa2");
  const minter = Ledger.selfAddress;
  const token_id = 1;

  console.log(`minting ${n} tokens to ${minter}`);

  const tokens = [{ token_id, owner: minter, amount: n }];

  await SmartFunction.call(
    new Request(`tezos://${fa2}/mint_new`, {
      method: "POST",
      body: JSON.stringify(tokens),
    }),
  );

  await fa2_balance_of(fa2, minter, token_id);

  const transfers = [
    {
      from: Ledger.selfAddress,
      transfers: [{ to: fa2, token_id, amount: 1 }],
    },
  ];

  for (let i = 0; i < n; i++) {
    await SmartFunction.call(
      new Request(`tezos://${fa2}/transfer`, {
        method: "POST",
        body: JSON.stringify(transfers),
      }),
    );
  }

  await fa2_balance_of(fa2, minter, token_id);

  return new Response();
}

export default handler;
