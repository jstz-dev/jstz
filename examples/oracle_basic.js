const handler = async () => {
  console.log("Fetching uuid4...");
  let response = fetch("http://httpbin.org/uuid");
  console.log("Running something else while waiting");
  return await response;
};

export default handler;
