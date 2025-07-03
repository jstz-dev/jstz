export default (request) => {
  const url = new URL(request.url);
  const iterations = url.searchParams.get("iterations") || 100;
  for (let i = 1; i <= iterations; ++i) {
    console.log(`Iteration: ${i}`);
  }
  return new Response();
};
