const logBalance = () => {
    console.log("Hello JS 👋");
    console.log(`Balance of tz4 account: ${Ledger.balance("tz4FENGt5zkiGaHPm1ya4MgLomgkL1k7Dy7q")}`);
}

logBalance();