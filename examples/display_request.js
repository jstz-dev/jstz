function handler(request) {
  try {
    console.log(`Hello from ${Ledger.selfAddress()} 👋`);
    console.log("Method: ", request.method);
    console.log("Referer:", request.headers.get("Referer"));
    console.log("Url:", request.url);
    let url = new URL(request.url);
    console.log("Url path:", url.pathname);
    console.log("Url hash:", url.hash);
    console.log("Url host:", url.host);
    console.log("Url hostname:", url.hostname);
    console.log("Url href:", url.href);
    console.log("Url origin:", url.origin);
    console.log("Url password:", url.password);
    console.log("Url pathname:", url.pathname);
    console.log("Url port:", url.port);
    console.log("Url protocol:", url.protocol);
    console.log("Url search:", url.search);
    console.log("Url username:", url.username);
    return new Response();
  } catch (error) {
    console.error(error);
    return Response.error(error);
  }
}
export default handler;
