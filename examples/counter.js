const KEY = "counter";

const handler = () => {
    let counter = Kv.get(KEY);
    console.log(`Counter: ${counter}`);
    if (counter === null) {
        counter = 0;
    } else {
        counter++;
    }
    Kv.set(KEY, counter);
}

export default handler;