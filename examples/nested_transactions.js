async function handler() {
  const nested_code2 = `
    export default async function (request) {

      let counter = Kv.get("Counter3");
      console.log("Counter 3: " + counter);
      if (counter === null) {
        counter = 0;
      } else {
        counter++;
      }
      Kv.set("Counter3", counter);
      console.log("Hello World");
      const arg = request.text();
      console.log(arg);
      return new Response();
    }
  `;

  const nested_code1 =
    `
    export default async function (request) {
      const nested_code2 = \`` +
    nested_code2 +
    `
      \`;
      console.log(nested_code2);
      let smartFunctionAddress = await SmartFunction.create(nested_code2);

      await fetch(
        new Request(\`tezos://\${smartFunctionAddress}/\`, {
          method: \`POST\`,
          body: \`Hello World\`,
        }),
      );
      let counter = Kv.get("Counter2");
      console.log("Counter 2: " + counter);
      if (counter === null) {
        counter = 0;
      } else {
        counter++;
      }
      Kv.set("Counter2", counter);
      const arg = request.text();
      console.log(arg);
      return new Response();
    }
  `;

  console.log(nested_code1);
  const smartFunctionAddress = await SmartFunction.create(nested_code1);

  await fetch(
    new Request(`tezos://${smartFunctionAddress}/`, {
      method: `POST`,
      body: `Hello World`,
    }),
  );

  let counter = Kv.get("Counter1");
  console.log(`Counter 1: ${counter}`);

  if (counter === null) {
    counter = 0;
  } else {
    counter++;
  }
  Kv.set("Counter1", counter);

  return new Response();
}

export default handler;
