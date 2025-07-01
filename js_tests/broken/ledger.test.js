const assert = (c) => {
  if (!c) {
    throw new Error(`Assertion failed`);
  }
};

const assertEquals = (a, b) => {
  if (a !== b) {
    throw new Error(`Expected ${a} to equal ${b}`);
  }
};

const test = (f) => {
  try {
    f();
  } catch (e) {
    console.error(`${f.name}: Test failed`);
    console.error(e);
  }
};

const ADDRESS1 = "tz1bRZ5kxRYXt1rnehFTRXsL1pYAJYqBTuNp";

const init = () => {
  if (globalThis.jstz !== undefined) {
    jstz.Account.setBalance(Ledger.selfAddress, 1000);
  } else {
    if (Ledger.balance(Ledger.selfAddress) === 0) {
      throw new Error(
        `Must fund address ${Ledger.selfAddress} before running tests`,
      );
    }
  }
};

const doTest = () => {
  test(function ledgerShouldBeANamespace() {
    const prototype1 = Object.getPrototypeOf(Ledger);
    const prototype2 = Object.getPrototypeOf(prototype1);

    assertEquals(Object.getOwnPropertyNames(prototype1).length, 0);
    assertEquals(prototype2, Object.prototype);
  });

  test(function ledgerSelfAddressContainsTz1Address() {
    const selfAddress = Ledger.selfAddress;

    assert(selfAddress.startsWith("tz1"));
  });

  test(function ledgerSelfAddressIsReadOnly() {
    try {
      Ledger.selfAddress = "foo";
      assert(false);
    } catch (e) {}
  });

  test(function ledgerBalanceIsANumber() {
    const balance = Ledger.balance(Ledger.selfAddress);

    assert(typeof balance === "number");
  });

  test(function ledgerBalanceThrowsErrorForInvalidAddress() {
    try {
      Ledger.balance("foo");
      assert(false);
    } catch (e) {}
  });

  test(function ledgerBalanceIsZeroForNewAddress() {
    const balance = Ledger.balance(ADDRESS1);

    assertEquals(balance, 0);
  });

  test(function ledgerBalanceIsNonZeroForFundedAddress() {
    const balance = Ledger.balance(Ledger.selfAddress);

    assert(balance > 0);
  });

  test(function ledgerTransferShouldTransferFunds() {
    const from = Ledger.selfAddress;
    const to = ADDRESS1;
    const amount = 100;

    const fromBalanceBefore = Ledger.balance(from);
    const toBalanceBefore = Ledger.balance(to);

    Ledger.transfer(to, amount);

    const fromBalanceAfter = Ledger.balance(from);
    const toBalanceAfter = Ledger.balance(to);

    assertEquals(fromBalanceAfter, fromBalanceBefore - amount);
    assertEquals(toBalanceAfter, toBalanceBefore + amount);
  });

  test(function ledgerTransferShouldThrowErrorForInsufficientFunds() {
    try {
      Ledger.transfer(ADDRESS1, 1000000000);
      assert(false);
    } catch (e) {}
  });

  test(function ledgerTransferShouldThrowErrorForInvalidAddress() {
    try {
      Ledger.transfer("bar", 100);
      assert(false);
    } catch (e) {}
  });

  test(function ledgerTransferShouldThrowErrorForNegativeAmount() {
    try {
      Ledger.transfer(ADDRESS1, -100);
      assert(false);
    } catch (e) {}
  });

  test(function ledgerTransferShouldThrowErrorForNonNumberAmount() {
    try {
      Ledger.transfer(ADDRESS1, "foo");
      assert(false);
    } catch (e) {}
  });
};

const handler = () => {
  init();
  doTest();
  return new Response();
};

export default handler;
