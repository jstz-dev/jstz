async function handler(request) {
  const method = request.method || "GET";
  console.log(`Method: ${method}`);
  return new Response(method);
}

export default handler;
