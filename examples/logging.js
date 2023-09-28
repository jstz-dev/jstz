
function handler () {
  try {
    console.log("Hello from handler 👋");
    console.info("About to call Sam's amazing new function.");
    console.warn("Not sure if Sam actually merged his PR yet");
    samsNewFunction();
  } catch (error) {
    console.error(error);
  }

  return new Response();
}

export default handler;
