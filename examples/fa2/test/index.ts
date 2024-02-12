import type { BalanceRequest, BalanceResponse, MintNew } from "../src/index";

// NOTE: When updating actor smart function, make sure to update the `ACTOR_FUNCTION_CODE` below
const ACTOR_FUNCTION_CODE =
  'async function u(c){let e=new URL(c.url),l=e.pathname;try{switch(l){case"/ping":return console.log("Hello from child smart function \u{1F44B}"),new Response("Pong!");case"/transfer":{let t=e.searchParams.get("to"),a=+e.searchParams.get("token_id"),s=+e.searchParams.get("amount"),o=e.searchParams.get("fa2"),n=[{from:Ledger.selfAddress,transfers:[{to:t,token_id:a,amount:s}]}];return await SmartFunction.call(new Request(`tezos://${o}/transfer`,{method:"POST",body:JSON.stringify(n)}))}case"/add_operator":{let t=e.searchParams.get("fa2"),a=JSON.parse(e.searchParams.get("tokens")),s=c.headers.get("Referer"),o=Ledger.selfAddress,n=a.map(d=>({operation:"add_operator",owner:o,operator:s,token_id:d}));return await SmartFunction.call(new Request(`tezos://${t}/update_operators`,{method:"PUT",body:JSON.stringify(n)}))}default:let r=`Unrecognised entrypoint ${l}`;return console.error(r),new Response(r,{status:404})}}catch(r){return console.error(r),Response.error()}}var g=u;export{g as default};';

async function createActors(n: number): Promise<Address[]> {
  let promises = new Array(n)
    .fill(0)
    .map(() => SmartFunction.create(ACTOR_FUNCTION_CODE));
  return await Promise.all(promises);
}

async function logBalances(fa2: Address, actor: Address[], tokens: number[]) {
  // 1. Create a list of BalanceRequests for each actor and token
  let requests: BalanceRequest[] = actor.flatMap((actor) =>
    tokens.map((token_id) => ({ owner: actor, token_id })),
  );

  // 2. Call the fa2 smart function
  let encodedRequests = TextEncoder.btoa(JSON.stringify(requests));

  let response = await SmartFunction.call(
    new Request(`tezos://${fa2}/balance_of?requests=${encodedRequests}`),
  );

  let balances = await response.json();

  // 3. Log the results
  balances.forEach((balance: BalanceResponse) => {
    console.log(
      `Address ${balance.request.owner} has ${balance.balance} tokens of type ${balance.request.token_id}`,
    );
  });
}

async function addSelfAsOperator(
  fa2: Address,
  actors: Address[],
  tokens: number[],
): Promise<Response[]> {
  // For each actor, add `Ledger.selfAddress` as an operator for each token in `fa2`
  let promises = actors.map((actor) =>
    SmartFunction.call(
      new Request(
        `tezos://${actor}/add_operator?fa2=${fa2}&tokens=${JSON.stringify(
          tokens,
        )}`,
      ),
    ),
  );
  return await Promise.all(promises);
}

async function mintTokens(
  fa2: Address,
  ...tokens: MintNew[]
): Promise<Response> {
  return await SmartFunction.call(
    new Request(`tezos://${fa2}/mint_new`, {
      method: "POST",
      body: JSON.stringify(tokens),
    }),
  );
}

async function transfer(
  fa2: Address,
  from: Address,
  to: Address,
  token_id: number,
  amount: number,
): Promise<Response> {
  return await SmartFunction.call(
    new Request(
      `tezos://${from}/transfer?fa2=${fa2}&to=${to}&token_id=${token_id}&amount=${amount}`,
    ),
  );
}

type StealRequest = {
  from: Address;
  tokens: { amount: number; token_id: number }[];
};
async function steal(
  fa2: Address,
  ...steals: StealRequest[]
): Promise<Response> {
  // 1. Create a list of transfers for each steal request
  let to = Ledger.selfAddress;
  let transfers = steals.map(({ from, tokens }) => ({
    from,
    transfers: tokens.map(({ amount, token_id }) => ({ to, token_id, amount })),
  }));

  // 2. Attempt to transfer the tokens
  return await SmartFunction.call(
    new Request(`tezos://${fa2}/transfer`, {
      method: "POST",
      body: JSON.stringify(transfers),
    }),
  );
}

async function runScenario(fa2: Address) {
  try {
    console.log("Creating 2 actors...");
    const actors = await createActors(2);
    console.log("Done!");

    const log = async () => {
      await logBalances(fa2, [...actors, Ledger.selfAddress], [1, 2]);
    };

    await log();

    console.log("Minting some tokens...");
    await mintTokens(
      fa2,
      { owner: actors[0], token_id: 1, amount: 3 },
      { owner: actors[1], token_id: 2, amount: 3 },
    );
    console.log("Done!");

    await log();

    console.log("Transfering 1 of token 1 from actor 0 to actor 1...");
    console.log("Transfering 1 of token 2 from actor 1 to actor 0...");
    await Promise.all([
      transfer(fa2, actors[0], actors[1], 1, 1),
      transfer(fa2, actors[1], actors[0], 2, 1),
    ]);
    console.log("Done!");

    await log();

    console.log(
      "Scenario smart function is attempting to steal tokens from actors...",
    );
    try {
      await steal(
        fa2,
        {
          from: actors[0],
          tokens: [
            { token_id: 1, amount: 2 },
            { token_id: 2, amount: 1 },
          ],
        },
        {
          from: actors[1],
          tokens: [
            { token_id: 1, amount: 1 },
            { token_id: 2, amount: 2 },
          ],
        },
      );
    } catch (error) {
      console.log(`Failed! 😭 Recieved error ${error}`);
      await logBalances(fa2, actors, [1, 2]);
    }

    console.log(
      "Scenario smart function is being added as an operator for all tokens for all actors...",
    );
    await addSelfAsOperator(fa2, actors, [1, 2]);
    console.log("Done!");

    console.info(
      "Scenario smart function is attempting to steal tokens from actors... (again)",
    );
    await steal(
      fa2,
      {
        from: actors[0],
        tokens: [
          { token_id: 1, amount: 2 },
          { token_id: 2, amount: 1 },
        ],
      },
      {
        from: actors[1],
        tokens: [
          { token_id: 1, amount: 1 },
          { token_id: 2, amount: 2 },
        ],
      },
    );
    console.log("Done!");

    await log();
  } catch (error) {
    console.error(error);
    throw error;
  }
}
async function handler(request: Request): Promise<Response> {
  let url = new URL(request.url);
  if (url.pathname == "/ping") {
    console.log("Hello from runner smart function 👋");
    return new Response("Pong");
  }

  let fa2 = url.searchParams.get("fa2") as string;
  await runScenario(fa2);

  return new Response();
}

export default handler;
