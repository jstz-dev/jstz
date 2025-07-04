const handler = async () => {
  console.log("Fetching uuid4");
  try {
    const responsePromise = fetch("http://httpbin.org/uuid");
    console.log("Running something else while waiting");
    const response = await responsePromise;
    if (!response.ok) {
      throw new Error(`HTTP error! Status: ${response.status}`);
    }
    const { uuid } = await response.json();
    console.log("UUID:", uuid);
    return new Response(uuid);
  } catch (error) {
    console.error("Failed to fetch UUID:", error.message);
  }
};

export default handler;
