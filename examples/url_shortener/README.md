# URL Shortener with Jstz

This project demonstrates how to create a URL shortener using the Jstz framework, which allows deploying smart functions on Tezos Smart Rollups.

## Table of Contents

- [Introduction](#introduction)
- [Setup](#setup)
- [Create the Smart Function](#create-the-smart-function)
- [Compile the Function](#compile-the-function)
- [Start the Sandbox](#start-the-sandbox)
- [Create an Account](#create-an-account)
- [Deploy the Function](#deploy-the-function)
- [Submit a URL to be Shortened](#submit-a-url-to-be-shortened)
- [Retrieve the Original URL](#retrieve-the-original-url)

## Introduction

This project uses the Jstz framework to create a smart function that shortens URLs and redirects to the original URL when accessed via the shortened link.

## Setup

1. **Clone the Jstz repository and install dependencies:**

   ```sh
   git clone https://github.com/jstz-dev/jstz.git
   cd jstz/examples
   ```

2. **Create a new directory and files for the URL shortener:**

   ```sh
   mkdir url-shortener
   cd url-shortener
   npm init -y
   npm install typescript @types/node --save-dev
   npx tsc --init
   touch index.ts
   ```

## Create the Smart Function

Create the `index.ts` file with the following content:

```typescript
// Utility function to generate a short code
function generateShortCode(): string {
  return Math.random().toString(36).substring(2, 8);
}

// Function to shorten the URL
async function shortenUrl(originalUrl: string): Promise<string> {
  const shortCode = generateShortCode();
  Kv.set(shortCode, { url: originalUrl } as UrlMapping);
  return shortCode;
}

// Function to get the original URL
function getOriginalUrl(shortCode: string): string | null {
  const data = Kv.get<UrlMapping>(shortCode);
  return data ? data.url : null;
}

// Handler function for the smart function
const handler = async (request: Request): Promise<Response> => {
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
      return new Response(new ArrayBuffer(0), {
        status: 301,
        headers: { Location: originalUrl },
      });
    } else {
      return new Response("URL not found", { status: 404 });
    }
  }
};

export default handler;
```

## Compile the Function

Ensure you are in the `url-shortener` directory and run the following command:

```sh
npx tsc
```

## Start the Sandbox

In a new terminal, start the Jstz sandbox:

```sh
jstz sandbox start
```

## Create an Account

```sh
jstz account create bob
```

## Deploy the Function

Deploy the smart function using the following command (replace `bob` with the account you created):

```sh
jstz deploy dist/index.js -n dev
```

## Submit a URL to be Shortened

Replace `<your-smart-function-address>` with the address returned after deployment.

```sh
jstz run tezos://<your-smart-function-address>/shorten --data '{"originalUrl":"https://beata.com"}' -n dev --request POST
```

## Retrieve the Original URL

Replace `<your-smart-function-address>` and `/<shortCode>` with the actual values:

```sh
jstz run tezos://<your-smart-function-address>/<shortCode> -n dev
```

## Summary

1. **Clone the repository and set up the environment.**
2. **Create and implement the smart function in TypeScript.**
3. **Compile the function using TypeScript.**
4. **Start the Jstz sandbox.**
5. **Create an account for deploying the smart function.**
6. **Deploy the smart function.**
7. **Submit URLs to be shortened and retrieve the original URLs using the short version.**

This process will create a fully functional URL shortener using the Jstz framework and its KV store.
