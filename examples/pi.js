export default () => {
  let n;
  let p = 0;
  for (n = 0; n < 1000; ++n) {
    let x = Math.random();
    let y = Math.random();
    if (x * x + y * y < 1) p += 4;
  }
  console.log(`pi = ${p / n}`);
  return new Response();
};
