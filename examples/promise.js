const doPromise = async () => {
    console.log('Hello JS from Promise');
}

const handler = async () => {
    await doPromise().then(res => {
        console.log('Hello from then!');
        return 42;
    });
    return new Response();
}

export default handler;