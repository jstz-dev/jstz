const capitalize = (word: string): string => {
  return word.charAt(0).toUpperCase() + word.slice(1);
};

const hello = (name: string): string => {
  return "Hello " + capitalize(name);
};

const handler = (request: Request): Response => {
  const url = new URL(request.url);
  const name = url.searchParams.get("name") || "World!";

  const msg = hello(name);
  console.log(`Message: ${msg}`);

  return new Response(msg);
};

export default handler;
