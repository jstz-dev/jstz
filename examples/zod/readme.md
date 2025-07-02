# Smart function with a router

You can create a smart function with 3rd-party JavaScript libraries.

This example shows how you can use third-party JavaScript libraries in Jstz smart functions. This example uses [itty-router](https://github.com/kwhitley/itty-router) to route requests and [zod](https://github.com/colinhacks/zod) to validate user input.
The smart function behaves as a server that stores data with two endpoints:

- `GET /user/:name`: returns information of a user.
- `POST /user`: creates a user with the given data. Input data must follow the schema below:

```
{
    name: A string composed of alphabets and space characters with at least 2 characters.
    age: A number.
    email: Optional. A string representing an email address.
}
```

## Setup

```
npm i
# Output file is dist/index.js
npm run build
```

## Demo

```
$ jstz deploy dist/index.js -n dev
Smart function deployed by user at address: KT1NLouAR2bkAXAV59TSn5DfzDPSMyNusqoa
Run with `jstz run jstz://KT1NLouAR2bkAXAV59TSn5DfzDPSMyNusqoa/ --network dev`
$ jstz run jstz://KT1NLouAR2bkAXAV59TSn5DfzDPSMyNusqoa/user/Jane --network dev
User not found
$ jstz run jstz://KT1NLouAR2bkAXAV59TSn5DfzDPSMyNusqoa/user --network dev -d '{"name": "Jane"}' -m POST
[
  {
    "code": "invalid_type",
    "expected": "number",
    "message": "Required",
    "path": [
      "age"
    ],
    "received": "undefined"
  }
]
$ jstz run jstz://KT1NLouAR2bkAXAV59TSn5DfzDPSMyNusqoa/user --network dev -d '{"name": "Jane", "age": 42}' -m POST
{
  "message": "user 'Jane' successfully created 🚀"
}
$ jstz run "jstz://KT1NLouAR2bkAXAV59TSn5DfzDPSMyNusqoa/user/Jane" --network dev
{
  "age": 42,
  "name": "Jane"
}
$ jstz run jstz://KT1NLouAR2bkAXAV59TSn5DfzDPSMyNusqoa/user --network dev -d '{"name": "Pam", "age": 42, "email": "pam.com"}' -m POST
[
  {
    "code": "invalid_string",
    "message": "Invalid email",
    "path": [
      "email"
    ],
    "validation": "email"
  }
]
$ jstz run jstz://KT1NLouAR2bkAXAV59TSn5DfzDPSMyNusqoa/user --network dev -d '{"name": "Pam", "age": 42, "email": "pam@email.com"}' -m POST
{
  "message": "user 'Pam' successfully created 🚀"
}
$ jstz run "jstz://KT1NLouAR2bkAXAV59TSn5DfzDPSMyNusqoa/user/Pam" --network dev
{
  "age": 42,
  "email": "pam@email.com",
  "name": "Pam"
}
```
