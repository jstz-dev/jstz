export default (request) => {
  const transferred_amount = request.headers.get("X-JSTZ-AMOUNT");
  console.log("transferred_amount", transferred_amount);
  if (transferred_amount !== "2000000") {
    return new Response();
  }
  return new Response(null, {
    headers: {
      "X-JSTZ-TRANSFER": "1000000",
    },
  });
};
