// Utility function to generate a short code
function generateShortCode() {
  return Math.random().toString(36).substring(2, 8);
}
// Function to shorten the URL
async function shortenUrl(originalUrl) {
  const shortCode = generateShortCode();
  Kv.set(shortCode, { url: originalUrl });
  return shortCode;
}
// Function to get the original URL
function getOriginalUrl(shortCode) {
  const data = Kv.get(shortCode);
  return data ? data.url : null;
}
// Handler function for the smart function
const handler = async (request) => {
  const url = new URL(request.url);
  const path = url.pathname;
  if (path === "/shorten" && request.method === "POST") {
    const { originalUrl } = await request.json();
    const shortCode = await shortenUrl(originalUrl);
    return new Response(
      JSON.stringify({ shortUrl: `tezos://${url.host}/${shortCode}` }),
      {
        headers: { "Content-Type": "application/json" },
      },
    );
  } else {
    const shortCode = path.slice(1);
    const originalUrl = getOriginalUrl(shortCode);
    if (originalUrl) {
      return new Response(null, {
        status: 301,
        headers: { Location: originalUrl },
      });
    } else {
      return new Response("URL not found", { status: 404 });
    }
  }
};
export default handler;
