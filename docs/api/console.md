# 🎮 console

An implementation of the Web standard [`console`](https://developer.mozilla.org/en-US/docs/Web/API/console) API suitable for logging and debugging `jstz` smart functions.

::: danger
⚠️ `jstz`'s implementation is not spec compliant ⚠️
:::

## Quick start

Accessible from the global scope, [`console.log`](#log) can be used to write a general logging message to the logs.

```typescript
console.log("Hello from JS 👋"); // Hello from JS 👋
```

For messages with a more specific purpose the following methods are provided:
[`console.info`](#info), [`console.warn`](#warn), [`console.error`](#error), [`console.debug`](#debug). For example:

```typescript
function parseURL(arg: unknown) {
  console.debug(`Running parseURL(${arg})`);
  if (typeof arg !== "string") {
    console.warn(`Expected a string, but received ${typeof arg}`);
    arg = arg.toString();
  }
  try {
    let result = new URL(arg);
    console.info(`Result: ${result}`);
    return result;
  } catch (error) {
    console.error(`Failed to parseURL: ${error}`);
  }
}
```

### Logging objects

When debugging `jstz` smart functions it is often useful to log the value of an object.
A common mistake is to try log an object directly, for example:

```typescript
console.log({ name: "Dave", age: 42 }); // [object Object]
```

This is because the `jstz` implementation of `console` doesn't support inspecting objects. A simple workaround is to use `JSON.stringify`:

```typescript
const dave = { name: "Dave", age: 42 };
console.log(JSON.stringify(dave)); // { "name": "Dave", "age": 42 }
```

### Assertions

[`console.assert`](#assert) will log an error message if its first argument is false.
If the first argument is true there will be no effect.

```typescript
function divide(a: number, b: number): number {
  console.assert(b != 0, "Trying to divide by 0");
  return a / b;
}
```

### Groups

[`console.group`](#group) facilitates the creation of nested log groups by introducing varying levels of indentation and assigning a group name. These groups can be neatly closed using [`console.groupEnd`](#groupEnd).

This feature is particularly handy for organizing logs to suit your needs. For instance, consider the following code, which conducts a brute force search for Pythagorean triples, showcasing the use of nested groups to reflect the level of organization within the logs.

```typescript
function pythagoreanTriples(limit: number = 100) {
  for (let x = 2; x <= limit; ++x) {
    console.group(`x = ${x}`);
    for (let y = 1; y < x; ++y) {
      console.group(`y = ${y}`);
      for (let z = x; z < x + y; ++z) {
        console.log(`trying z = ${z}`);
        if (x * x + y * y === z * z) {
          console.log(`Success! ${x}^2 + ${y}^2 = ${z}^2`);
          break;
        }
      }
      console.groupEnd();
    }
    console.groupEnd();
  }
}
```

Running `pythagoreanTriples(2)` will produce the following output:

```
group: x = 2
  group: y = 1
    trying z = 2
```

## Instance Methods

### `console.log(...message : unknown[]) : void`{#log}

Outputs a general logging message.
String reresentations of each of the arguments will be concatenated, separated by spaces and written to the logs.

### `console.info(...message : unknown[]) : void`{#info}

Outputs a informative logging message.
String reresentations of each of the arguments will be concatenated, separated by spaces and written to the logs.

### `console.warn(...message : unknown[]) : void`{#warn}

Outputs a warning logging message.
String reresentations of each of the arguments will be concatenated, separated by spaces and written to the logs.

### `console.error(...message : unknown[]) : void`{#error}

Outputs a warning message.
String reresentations of each of the arguments will be concatenated, separated by spaces and written to the logs.

### `console.assert(assertion: unknown, ...message : unknown[]) : void`{#assert}

Outputs an error message if the first argument is falsy.
String reresentations of each of the arguments will be concatenated, separated by spaces and written to the logs.
Has no effect if the first argument is truthy.

### `console.debug(...message : unknown[]) : void`{#debug}

Outputs a debug logging message.
String reresentations of each of the arguments will be concatenated, separated by spaces and written to the logs.

### `console.group(...label : unknown[]) : void`{#group}

Begins a group and pushes the label to the group stack.
The group label will consist of the string reresentations of each of the arguments, concatenated and separated by spaces.
Subsequent messages will be indented at the level of the group.

### `console.groupCollapsed(...message : unknown[]) : void`

This is provided for compatibility with existing frameworks.
The behaviour is identical to [`console.group(...)`](#group).

### `console.groupEnd() : void`{#groupEnd}

Closes the current group and pops the group stack.
Has no effect if the group stack is empty.

### `console.clear() : void`{#clear}

Provided for compatibility with existing frameworks.
Closes all groups in the current group stack.
