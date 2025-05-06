// The (long) URL that is stored in the KV store, mapped to a short code.
type ShortCodeData = { url: string };

// Utility function to generate a short code
function generateShortCode() {
  return Math.random().toString(36).substring(2, 8);
}

// Function to shorten the URL
function shortenUrl(originalUrl: string) {
  const shortCode = generateShortCode();
  Kv.set(shortCode, { url: originalUrl });
  return shortCode;
}

// Function to get the original URL
function getOriginalUrl(shortCode: string) {
  const data: ShortCodeData | null = Kv.get(shortCode);
  return data?.url;
}

// Handler function for the smart function
const handler = async (request: Request): Promise<Response> => {
  const url = new URL(request.url);
  const path = url.pathname;
  if (path === "/shorten" && request.method === "POST") {
    const { originalUrl } = await request.json();
    const shortCode = shortenUrl(originalUrl);
    return new Response(
      JSON.stringify({ shortUrl: `jstz://${url.host}/${shortCode}` }),
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
