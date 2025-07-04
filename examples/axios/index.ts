import axios from "axios";

const handler = async (request: Request) => {
  const { echoSf } = await request.json();
  let externalCall = axios.get("http://httpbin.org/uuid");
  let echo = await axios.post(`jstz://${echoSf}`, { body: "Hello world!" });
  let response = await externalCall;
  let { uuid } = response.data;
  console.log("UUID " + uuid);
  console.log(echo.data);
  return new Response("OK!");
};

export default handler;
