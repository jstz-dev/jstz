const handler = async () => {
    try {
    // Constructor 1
    {
        const oldRequest = new Request(
            "tezos://github.tez/tezos/issues/12959",
            { headers: { "From": "webmaster@example.org" } },
        );
        console.log(`Actual: ${oldRequest.headers.get("From")}, Expected: webmaster@example.org`);
    }

    // Constructor 2
    {
        const myHeaders = new Headers();
        myHeaders.append("Content-Type", "image/jpeg");

        const myOptions = {
            method: "GET",
            headers: myHeaders,
        };

        const myRequest = new Request("tezos://flowers.tez/flowers.jpg", myOptions);
        console.log(`Actual: ${myRequest.headers.get("Content-Type")}, Expected: image/jpeg`);
        console.log(`Method: ${myRequest.method}, Expected: GET`);
    }

    // bodyUsed
    {
        const request = new Request("tezos://sam.tez/myEndpoint", {
            method: "POST",
            body: "Hello world",
        });

        console.log(`Actual: ${request.bodyUsed}, Expected: false`);

        await request.text();

        console.log(`Actual: ${request.bodyUsed}, Expected: true`);
    }

    // Headers 1
    {
        const myHeaders = new Headers();
        myHeaders.append("Content-Type", "image/jpeg");

        const myInit = {
            method: "GET",
            headers: myHeaders,
        };

        const myRequest = new Request("tezos://flowers.tez/flowers.jpg", myInit);

        const myContentType = myRequest.headers.get("Content-Type");
        console.log(`Actual: ${myContentType}, Expected: image/jpeg`);
    }

    // Method
    {
        const myRequest = new Request("tezos://flowers.tez/flowers.jpg");
        const myMethod = myRequest.method; // GET
        console.log(`Actual: ${myMethod}, Expected: GET`);
    }

    // Url
    {
        const myRequest = new Request("tezos://flowers.tez/flowers.jpg");
        const myURL = myRequest.url;
        console.log(`Actual: ${myURL}, Expected: tezos://flowers.tez/flowers.jpg`);
    }

    // Text
    {
        const text = "Hello world";

        const request = new Request("tezos://alistair.tez/myEndpoint", {
            method: "POST",
            body: text,
        });

        const reqText = await request.text();
        console.log(`Text: ${reqText}`);
    }
} catch (e) {
    console.error(e)
}
    return new Response();
}

export default handler;