const handler = async () => {
  console.log("Fetching uuid4...");
  const openaiPayload = {
    model: "gpt-4o-mini",
    messages: [{ role: "user", content: "Hello, how are you?" }],
  };
  const responsePromise = fetch("https://api.openai.com/v1/chat/completions", {
    method: "POST",
    headers: {
      Authorization: `Bearer <YOUR_OPENAI_API_KEY>`,
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(openaiPayload),
  });
  console.log("Running something else while waiting");
  return await responsePromise;
};

export default handler;
