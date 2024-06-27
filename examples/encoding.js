function encodeDecode(str) {
  console.log(`encoding "${str}"`);
  let b64 = btoa(str);
  console.info(b64);
  console.log(`decoding "${b64}"`);
  let back = atob(b64);
  console.info(back);
}

export default () => {
  let test_string;
  let test_strings = [
    "hello world",
    JSON.stringify({ foo: "bar" }),
    "👋 from JSꜩ 🎉",
  ];
  try {
    test_strings.forEach((str) => {
      test_string = str;
      encodeDecode(str);
    });
  } catch (error) {
    console.error(`error decoding ${test_string}: ${error}`);
    throw error;
  }
  return new Response();
};
