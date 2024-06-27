const handler = () => {
  // Constructor 1
  {
    const myHeaders = new Headers();
    myHeaders.append("Content-Type", "image/jpeg");
    console.log(
      `Actual: ${myHeaders.get("Content-Type")}, Expected: image/jpeg`,
    );
  }

  // Constructor 2
  {
    const httpHeaders = {
      "Content-Type": "image/jpeg",
      "X-My-Custom-Header": "Zeke are cool",
    };
    const myHeaders = new Headers(httpHeaders);
    console.log(
      `Actual: ${myHeaders.get("Content-Type")}, Expected: image/jpeg`,
    );
    console.log(
      `Actual: ${myHeaders.get("X-My-Custom-Header")}, Expected: Zeke are cool`,
    );
  }

  // Constructor 3
  {
    const httpHeaders = {
      "Content-Type": "image/jpeg",
      "X-My-Custom-Header": "Zeke are cool",
    };
    const myHeaders = new Headers(httpHeaders);
    const secondHeadersObj = new Headers(myHeaders);
    console.log(
      `Actual: ${secondHeadersObj.get("Content-Type")}, Expected: image/jpeg`,
    );
  }

  // Append
  {
    const myHeaders = new Headers();

    myHeaders.append("Content-Type", "image/jpeg");
    console.log(
      `Actual: ${myHeaders.get("Content-Type")}, Expected: image/jpeg`,
    );

    myHeaders.append("Accept-Encoding", "deflate");
    myHeaders.append("Accept-Encoding", "gzip");
    console.log(
      `Actual: ${myHeaders.get("Accept-Encoding")}, Expected: deflate, gzip`,
    );
  }

  // Delete
  {
    const myHeaders = new Headers();

    myHeaders.append("Content-Type", "image/jpeg");
    console.log(
      `Actual: ${myHeaders.get("Content-Type")}, Expected: image/jpeg`,
    );

    myHeaders.delete("Content-Type");
    console.log(`Actual: ${myHeaders.get("Content-Type")}, Expected: null`);
  }

  // Has
  {
    const myHeaders = new Headers();
    myHeaders.append("Content-Type", "image/jpeg");
    console.log(`Actual: ${myHeaders.has("Content-Type")}, Expected: true`);
    console.log(`Actual: ${myHeaders.has("Accept-Encoding")}, Expected: false`);
  }

  // Set
  {
    const myHeaders = new Headers();
    myHeaders.append("Content-Type", "image/jpeg");
    console.log(
      `Actual: ${myHeaders.get("Content-Type")}, Expected: image/jpeg`,
    );
    myHeaders.set("Content-Type", "text/html");
    console.log(
      `Actual: ${myHeaders.get("Content-Type")}, Expected: text/html`,
    );
  }

  // Keys
  {
    const httpHeaders = {
      "Content-Type": "image/jpeg",
      "Content-Encoding": "gzip",
      "Content-Length": "42",
      Accept: "text/html",
    };
    const myHeaders = new Headers();
    myHeaders.append("Accept-Encoding", "deflate");
    let lastKey = "";
    for (const key of myHeaders.keys()) {
      console.log(key);

      // keys should be strictly increasing
      console.assert(lastKey < key);
      lastKey = key;
    }
  }

  // Values
  {
    const httpHeaders = {
      "Content-Type": "image/jpeg",
      "Content-Encoding": "gzip",
      "Content-Length": "42",
      Accept: "text/html",
    };
    const myHeaders = new Headers(httpHeaders);
    for (const value of myHeaders.values()) {
      console.log(value);
    }
  }

  // Key-Values
  {
    const httpHeaders = {
      "content-type": "image/jpeg",
      "content-encoding": "gzip",
      "content-length": 42,
      accept: "text/html",
    };
    const myHeaders = new Headers(httpHeaders);
    for (const [key, value] of myHeaders) {
      console.log(`key: ${key}, value: ${value}, expect: ${httpHeaders[key]}`);
      console.assert(httpHeaders[key] == value);
    }
    myHeaders.forEach((value, key) => {
      console.log(`key: ${key}, value: ${value}, expect: ${httpHeaders[key]}`);
      console.assert(httpHeaders[key] == value);
    });
  }

  return new Response();
};

export default handler;
