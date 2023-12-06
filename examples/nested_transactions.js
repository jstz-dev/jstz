export default async () => {
  /*
  const nested_code1 =
    "\
    export default async () => {\
      const nested_code2 = ```\
        export default async () => {\
          // Increment counter \
          let counter = Kv.get('Counter3');\
          console.log(`Counter 3: ${counter}`);\
          if (counter === null) {\
            counter = 0;\
          } else {\
            counter++;\
          }\
          Kv.set('Counter3', counter);\
          \
          return new Response();\
        };\
      ```\
    \
    const subcontractAddress = await Contract.create(nested_code2);\
    let response = await Contract.call(subcontractAddress);\
    \
    // Increment counter \
    let counter = Kv.get('Counter2');\
    console.log(`Counter 2: ${counter}`);\
    if (counter === null) {\
      counter = 0;\
    } else {\
      counter++;\
    }\
    Kv.set('Counter2', counter);\
    \
      return new Response();\
    };\
    ";

  try {
    const subcontractAddress = await Contract.create(nested_code1);
    let response = await Contract.call(subcontractAddress);
  } catch (error) {
    console.error(error);
  }
  */

  // Increment counter
  let counter = Kv.get("Counter1");
  console.log(`Counter 1: ${counter}`);
  if (counter === null) {
    counter = 0;
  } else {
    counter++;
  }
  Kv.set("Counter1", counter);

  return new Response();
};
