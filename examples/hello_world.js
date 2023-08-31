const handler = async () => {
    console.log("Hello JS 👋")
    console.log(`My address is ${Ledger.selfAddress()}`);

    await Contract.call("tz4FENGt5zkiGaHPm1ya4MgLomgkL1k7Dy7q", `
const handler = () => {
    console.log("Hello from sub contract call 👋")
    console.log(\`My address is \${Ledger.selfAddress()}\`)
}

export default handler;
`);

    console.log("The root contract has control again!");
    console.log(`And to confirm, my address is ${Ledger.selfAddress()}`);
}

export default handler;