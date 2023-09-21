

let contractCode = `
const handler = async (request) => {
    try {
        const message = await request.text()
        console.log(message);
        console.log(\`My address is \${Ledger.selfAddress()}\`)
        const response = new Response("Success!");
        return response;
    } catch (error) { console.error("subcontract error", error)
                      return Response.error(error)
                    }
}
export default handler`

const handler = async () => {
    console.log("Hello JS 👋")
    console.log(`My address is ${Ledger.selfAddress()}`);

    try {
        const newContract = Ledger.createContract(contractCode);
        console.log("created new contract with address", newContract);
        const url = `tezos://sam.tez/myEndPoint`
        const request = new Request(url, {
            method: "POST",
            body: "Hello from Subcontract 👋"
        });

        const response = await Contract.call(newContract, request);
        console.log(await response.text());

    } catch(error) {
        console.error(error);
        return Response.error("😿");
    }

    console.log("The root contract has control again!");
    console.log(`And to confirm, my address is ${Ledger.selfAddress()}`);
    const response = new Response("😸");
    return response;
}

export default handler;
