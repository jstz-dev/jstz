const handler = () => {
  console.log(`${new Date().toJSON()}`);
  return new Response();
};

export default handler;
