# Writing and Deploying a SmartFunction

## Deploying your first contract


### A very simple contract
JsTz smart functions are modeled on http servers. These designed to be more familiar to most developers and to integrate better with the off chain part of your dApp.
Let's start with a very simple smart function. A smart function is simply an ECMAScript module with a default export. The export should be a function that accepts a `Request` object and returns either a response or a `Promise` containing a response. Most real world contracts will use promises, but to get started we'll just define a single endpoint and in accordance with custom and tradition say "Hello World";

``` javascript
export default (request) => {
  const url = new URL(request.url);
  const path = url.pathname;
  if (path === "/say_hello") {
    console.log("Hello World! 👋");
    return new Response();
  }
  const errorMessage = `No entrypoint ${path}`;
  console.error(errorMessage);
  return new Response(errorMessage, {status: 404});
}
```

For now let's just save this in a file. Pick any name you like, but I'm going to guess you picked `index.js`.

### Deploying to the sandbox
Hopefully you've got everything installed. If not have a look at our installation guide. 
We're building a proper CLI which wi
#### Start the sandbox
First we'll start the sandbox envirronment. We're building a proper CLI but for now we've just slung a bunch of bash scripts together and hoped for the best. We'll update the documentation when we're ready to release it so that you can have the smooth, hassle free development experience you deserve.

To start the sandbox we can use the same commands we used when you checked your installation was set up properly.

``` sh
#eval $(./scripts/sandbox.sh )
tail -f logs/rollup.log
```
When the logs start rolling you're ready to deploy, first you'll need a valid address that you can pretend is a real blockchain account.
``` sh
tz4=tz492MCfwp9V961DhNGmKzD642uhU8j6H5nB
```

In a separate window run the following command which will display the JsTz console. (We're sorry this is bad the CLI will be great we promise)

``` sh
./scripts/jstz.sh view-console
```
It will sit there doing nothing for a while just waiting to be sent a message.

#### Deploy your contract
First you need to get your contract into the sandbox. This is called deploying the contract. 

``` sh
jstz deploy-contract --self-address $tz4 < index.js
```
You should see something like this in your console window.

``` sh
[📜] Contract created: tz4RQn8huKS9KLoHZxWkghytBzxwbn84JnSb
```
This is the contract address. In JsTz these work analogously to ip addresses. 
You may or may not see the same address that we've printed above. If you don't don't worry about it, you're contract is still there, but make sure you use the address you've just created. 
Rather than type this in all the time we can define a shell variable to make things a bit more readable.
``` sh
my_contract=tz4RQn8huKS9KLoHZxWkghytBzxwbn84JnSb
```

#### Run your contract.
If you've got this far your first contract is now succesfully deployed to the sandbox. 
To run it we need to sent it a url. For now we need to explicitly send the referer. We promise we'll sort that all out in later versions. For now we can only send a url string as a `GET` request. Contracts can send each other different sorts of requests and we'll accept full uri's soon. 

We build the url is a similar way to an ordinary http request. 
* The url scheme must be `tezos`.
* The hostname is the address of the contract you are calling.
* The path should identify the endpoint of your contract.
* Search parameters and hash properties are optional but can be handled by the contract.

So to call our hello world contract type this.
``` sh
jstz run-contract --referer $tz4 "tezos://${my_contract}/say_hello"
```

If everything worked you should see the message in the logs. 





## Don't forget to stop the sandbox
``` sh
octez-reset
```


