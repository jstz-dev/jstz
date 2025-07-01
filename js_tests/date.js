const handler = () => {
  console.log(`${new Date().toJSON()}`);
  console.log(`${Date.now()}`);
  console.log(`${Date()}`);
  return new Response();
};

export default handler;
