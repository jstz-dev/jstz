const handler = () => {
    // Accessing components
    {
        let addr = new URL("https://developer.mozilla.org/en-US/docs/Web/API/URL_API");
        let host = addr.host;
        let path = addr.pathname;
        console.log(`Host: ${host}, Path: ${path}`);
    }

    // Changing URL
    {
        let myUsername = "someguy";
        let addr = new URL("https://example.com/login");
        addr.username = myUsername;
        console.log(`Href: ${addr.href}`);
    }

    // Queries
    {
        let addr = new URL("https://example.com/login?user=someguy&page=news");
        let user = addr.searchParams.get("user");
        let page = addr.searchParams.get("page");
        console.log(`User: ${user}, Page: ${page}`);
    }

    return new Response();
}

export default handler;