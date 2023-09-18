const incrementKey = (key, err = false) => `
const handler = async () => {
    const value = Kv.get("${key}") + 1;
    Kv.set("${key}", value);
    console.log("Hello from subcontract: ${key}", value);
    if (${err}) { throw "error" }
}
export default handler`;


const myAddress = Ledger.selfAddress();
const handler = async () => {
    console.log(`my address is ${myAddress}`)
    console.log(Kv.get('nonce'))

    Kv.set('nonce', 42);

    await Contract.call(myAddress, incrementKey('nonce'));
    try {
        await Contract.call(myAddress, incrementKey('nonce', true))
    } catch (error) {
        console.error(error);
    }
    Contract.call(myAddress, incrementKey('nonce'))

    const value = Kv.get("nonce");
    console.log("Hello from contract: nonce", value);
    return Contract.call(myAddress, incrementKey('nonce'))

}
export default handler;
