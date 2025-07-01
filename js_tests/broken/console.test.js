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

const doTest = () => {
  // console.test1.mjs

  test(function consoleShouldBeANamespace() {
    const prototype1 = Object.getPrototypeOf(console);
    const prototype2 = Object.getPrototypeOf(prototype1);

    assertEquals(Object.getOwnPropertyNames(prototype1).length, 0);
    assertEquals(prototype2, Object.prototype);
  });

  test(function consoleAssertShouldNotThrowError() {
    console.assert(true);
    let hasThrown = undefined;
    try {
      console.assert(false);
      hasThrown = false;
    } catch {
      hasThrown = true;
    }
    assertEquals(hasThrown, false);
  });

  test(function consoleStringifyComplexObjects() {
    console.log("foo");
    console.log(["foo", "bar"]);
    console.log({ foo: "bar" });
  });

  test(function consoleStringifyQuotes() {
    console.log(["\\"]);
    console.log(['\\,"']);
    console.log([`\\,",'"`]);
    console.log(["\\,\",',`"]);
  });

  test(function consoleStringifyLongStrings() {
    const veryLongString = "a".repeat(120);
    console.log({ veryLongString });
    console.log(veryLongString);
  });

  test(function consoleStringifyCycle() {
    const y = { a: { b: {} }, foo: { bar: {} } };
    y.a.b = y.a;
    y.foo.bar = y.foo;
    console.log(y);
  });

  test(function consoleStringifyClassesAndSubclasses() {
    class Base {
      a = 1;
      m1() {}
    }

    class Child extends Base {
      b = 2;
      m2() {}
    }

    const x = { base: new Base(), child: new Child() };
    const xCstr = { base: Base, child: Child };
    console.log(x);
    console.log(xCstr);
  });

  test(function consoleStringifyObject() {
    const obj = {
      num: 1,
      bool: true,
      str: "a",
      method() {},
      async asyncMethod() {},
      *generatorMethod() {},
      un: undefined,
      nu: null,
      arrowFunc: () => {},
    };

    console.log(obj);
  });

  test(function consoleStringifyIntrinsics() {
    console.log(1);
    console.log(-0);
    console.log(1n);
    console.log("s");
    console.log(new Number(1));
    console.log(new Number(-0));
    console.log(Object(1n));
    console.log(new Boolean(true));
    console.log(new String("jstz"));
    console.log(/[0-9]*/);
    console.log(new Date("2024-02-03T11:31:00.00Z"));
    console.log(new Set([1, 2, 3]));
    console.log(new Set([1, 2, 3]).values());
    console.log(new Set([1, 2, 3]).entries());
    console.log(
      new Map([
        ["a", 1],
        ["b", 2],
        ["c", 3],
      ]),
    );
    console.log(
      new Map([
        ["a", 1],
        ["b", 2],
        ["c", 3],
      ]).keys(),
    );
    console.log(
      new Map([
        ["a", 1],
        ["b", 2],
        ["c", 3],
      ]).values(),
    );
    console.log(
      new Map([
        ["a", 1],
        ["b", 2],
        ["c", 3],
      ]).entries(),
    );
    console.log(new WeakSet());
    console.log(new WeakMap());
    console.log(Symbol(1));
    console.log(Object(Symbol(1)));
    console.log(null);
    console.log(undefined);

    class A {
      a = 1;
    }
    console.log(new A());
    console.log(A);

    class B extends A {
      b = 2;
    }

    console.log(new B());
    console.log(B);

    console.log(function f() {});
    console.log(async function af() {});
    console.log(function* gf() {});
    console.log(async function* agf() {});

    console.log(new Uint8Array([1, 2, 3]));
    console.log(Uint8Array.prototype);

    console.log({ a: { b: { c: { d: new Set([1]) } } } });

    console.log(console.__proto__);
    console.log(JSON);
  });

  test(function consoleStringifyLargeObject() {
    const obj = {
      a: 2,
      o: {
        a: "1",
        b: "2",
        c: "3",
        d: "4",
        e: "5",
        f: "6",
        g: 10,
        asd: 2,
        asda: 3,
        x: { a: "asd", x: 3 },
      },
    };

    console.log(obj);
  });

  // console.test2.mjs

  test(function consoleStringifyPromises() {
    const pendingPromise = new Promise((_res, _rej) => {});
    console.log(pendingPromise);

    const resolvedPromise = Promise.resolve("Resolved!");
    console.log(resolvedPromise);

    const rejectPromise = Promise.reject("Rejected!");
    console.log(rejectPromise);
  });

  test(function consoleStringifyWithIntegerFormatSpecifier() {
    // expect %i
    console.log("%i");
    // expect 42
    console.log("%i", 42.0);
    // expect 42
    console.log("%i", 42);
    // expect 42
    console.log("%i", "42");
    // expect 0
    console.log("%i", 0.5);
    // expect -0
    console.log("%i", -0.5);
    // expect NaN
    console.log("%i", "");
    // expect NaN
    // currently fails
    try {
      console.log("%i", Symbol());
    } catch {
      console.error("Symbol() is not supported for %i");
    }
    // expect NaN
    console.log("%i", null);
    // expect 42 43
    console.log("%i %d", 42, 43);
    // expect 42 %i
    console.log("%d %i", 42);
    // UB
    console.log("%d", 12345678901234567890123);
    // expect 12345678901234567890123n
    console.log("%i", 12345678901234567890123n);
  });

  test(function consoleStringifyWithFloatFormatSpecifier() {
    // expect %f
    console.log("%f");
    // expect 42
    console.log("%f", 42.0);
    // expect 42
    console.log("%f", 42);
    // expect 42
    console.log("%f", "42");
    // expect 0.5
    console.log("%f", 0.5);
    // expect -0.5
    console.log("%f", -0.5);
    // expect 3.141592653589793
    console.log("%f", Math.PI);
    // expect NaN
    console.log("%f", "");
    // expect NaN
    // currently fails
    try {
      console.log("%f", Symbol());
    } catch {
      console.error("Symbol() is not supported for %f");
    }
    // expect 5
    try {
      console.log("%f", 5n);
    } catch {
      console.error("BigInt is not supported for %f");
    }
    // expect 42 43
    console.log("%f %f", 42, 43);
    // expect 42 %f
    console.log("%f %f", 42);
  });

  test(function consoleStringifyWithStringFormatSpecifier() {
    // expect %s
    console.log("%s");
    // expect undefined
    console.log("%s", undefined);
    // expect foo
    console.log("%s", "foo");
    // expect 42
    console.log("%s", 42);
    // expect 42
    console.log("%s", "42");
    // expect 42 %s
    console.log("%s %s", 42);
    // expect Symbol(foo)
    try {
      console.log("%s", Symbol("foo"));
    } catch {
      console.error("Symbol() is not supported for %s");
    }
  });

  test(function consoleStringifyWithObjectFormatSpecifier() {
    // expect %o
    console.log("%o");
    // expect 42
    console.log("%o", 42);
    // expect "foo"
    console.log("%o", "foo");
    // expect { a: 1 }
    console.log("%o", { a: 1 });
    // expect { a: { b: { c: { d: [1, 2, 3] } } } }
    console.log("%o", { a: { b: { c: { d: [1, 2, 3] } } } });
    console.log("%O", { a: { b: { c: { d: [1, 2, 3] } } } });
    // expect { a: 1 } %o
    console.log("%o %o", { a: 1 });
  });

  test(function consoleDetachedLog() {
    const log = console.log;
    const error = console.error;
    const debug = console.debug;
    const warn = console.warn;
    const info = console.info;
    const assert = console.assert;
    const group = console.group;
    const groupCollapsed = console.groupCollapsed;
    const groupEnd = console.groupEnd;
    const clear = console.clear;
    log("Hello world");
    debug("Hello world");
    info("Hello world");
    warn("Hello world");
    error("Hello world");
    assert(true);
    group("Hello world");
    groupEnd();
    clear();
    groupCollapsed("Hello world");
    groupEnd();
  });

  test(function consoleError() {
    console.error("Hello world");
  });

  test(function consoleGroup() {
    console.group("1");
    console.log("2");
    console.group("3");
    console.log("4");
    console.groupEnd();
    console.groupEnd();
    console.log("5");
    console.log("6");
  });

  test(function consoleDebug() {
    console.debug("Hello world");
  });

  test(function consoleInfo() {
    console.info("Hello world");
  });

  test(function consoleWarn() {
    console.warn("Hello world");
  });
};

const handler = () => {
  doTest();
  return Response();
};

export default handler;
