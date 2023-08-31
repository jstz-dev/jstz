const doPromise = async () => {
    console.log('Hello JS from Promise');
}

const handler = () => {
    doPromise().then(res => {
        console.log('Hello from then!');
        return 42;
    });
}

export default handler;