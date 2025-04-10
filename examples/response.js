const handler = async () => {
  // Constructor
  {
    const myOptions = { status: 420 };
    const myResponse = new Response("Hello World", myOptions);

    console.assert(!myResponse.bodyUsed);
    const myText = await myResponse.text();
    console.assert(myResponse.bodyUsed);
    console.log(`Actual: ${myText}, Expected: Hello World`);
    console.log(`Actual: ${myResponse.status}, Expected: 420`);
  }

  // Ok
  {
    const myResponse = new Response("Hello World");
    console.log(`Actual: ${myResponse.ok}, Expected: true`);

    const myFailedResponse = new Response("Goodbye World", { status: 500 });
    console.log(`Actual: ${myFailedResponse.ok}, Expected: false`);
  }

  // error
  {
    const myResponse = Response.error();
    console.log(`Actual: ${myResponse.ok}, Expected: false`);
  }

  // json
  {
    const jsonResponse = Response.json({ my: "data" });
    const resJson = await jsonResponse.json();
    console.log(`Actual: ${resJson.my}, Expected: "data"`);
  }

  // Redirect
  {
    const myResponse = Response.redirect(`jstz://${Ledger.selfAddress}`);
    console.log(myResponse.url);
  }

  return new Response();
};

export default handler;
