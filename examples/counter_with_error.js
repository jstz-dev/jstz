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
    if (counter == 4) {
        throw new Error("counter too high");
    }
}

export default handler;
