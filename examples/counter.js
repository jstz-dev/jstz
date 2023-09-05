const KEY = "counter";

const handler = () => {
    let counter = Storage.get(KEY);
    console.log(`Counter: ${counter}`);
    if (counter === null) {
        counter = 0;
    } else {
        counter++;
    }
    Storage.set(KEY, counter);
}

handler();