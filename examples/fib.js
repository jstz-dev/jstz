function fibonacci(n) {
  if (n <= 1) {
    return n;
  } else {
    return fibonacci(n - 1) + fibonacci(n - 2);
  }
}

function handler(request) {
  const url = new URL(request.url);
  const n = url.searchParams.get("n") || 0;
  const result = fibonacci(n);
  return new Response(result.toString());
}

export default handler;
