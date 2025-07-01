const handler = () => {
  console.log("log");
  console.debug("debug");
  console.info("info");
  console.warn("warn");
  console.error("error");
  console.assert(1 === 2);
  return new Response();
};

export default handler;
