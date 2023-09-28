const NAMESPACE = "ACCOUNTS";
const ACCOUNT = "ajob410";

// Account
// { firstName, lastName, nonce }

const handler = () => {
    const key = `${NAMESPACE}/${ACCOUNT}`;
    let account = Kv.get(key);
    console.log(`Fetching account: ${JSON.stringify(account)}`);
    if (account === null) {
        account = { firstName: "Alistair", lastName: "O'Brien", nonce: 0 };
    } else {
        // increment nonce
        account.nonce++;
    }
    Kv.set(key, account);
    return new Response()
}

export default handler;