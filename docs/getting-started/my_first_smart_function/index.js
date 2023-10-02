export default (request) => {
  const url = new URL(request.url);
  const path = url.pathname;
  if (path === "/say_hello") {
    console.log("Hello World! 👋");
    return new Response();
  }
  const errorMessage = `No entrypoint ${path}`;
  console.error(errorMessage);
  return new Response(errorMessage, {status: 404});
}
