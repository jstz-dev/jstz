# Building smart functions

Smart functions are written and run in ordinary TypeScript, but you must still build them with the Jstz dependencies before deploying them.

For examples of smart function projects, see the examples folder: https://github.com/jstz-dev/jstz/tree/main/examples.

Follow these steps to create and build a TypeScript project for your smart function:

1. Create a `package.json` file appropriate for a TypeScript project.
   You can use the `npm init` command, the `yarn init` command, or create a `package.json` file manually to describe the project.

1. Add the `@jstz-dev/jstz` dependency and `esbuild` build dependencies.

1. Add a build script that looks like this, with the name of your smart function source file in place of the variable `<SOURCE_FILE>`:

   ```bash
   esbuild <SOURCE_FILE> --bundle --format=esm --target=esnext --minify --outfile=dist/index.js
   ```

   Here is an example of a `package.json` file for a smart function project:

   ```json
   {
     "name": "my-smart-function",
     "authors": "",
     "version": "0.0.0",
     "main": "index.ts",
     "dependencies": {
       "@jstz-dev/jstz": "^0.0.0"
     },
     "devDependencies": {
       "esbuild": "^0.20.2"
     },
     "scripts": {
       "build": "esbuild index.ts --bundle --format=esm --target=esnext --minify --outfile=dist/index.js"
     }
   }
   ```

1. Add a `tsconfig.json` file to specify how TypeScript builds the file, including accessing types for Jstz code, as in this example:

   ```json
   {
     "compilerOptions": {
       "lib": ["esnext"],
       "module": "esnext",
       "target": "esnext",
       "strict": true,
       "moduleResolution": "node",
       "types": ["@jstz-dev/types"]
     },
     "exclude": ["node_modules"]
   }
   ```

1. Install the dependencies by running `npm install` or `yarn install`.

1. Add the code of your smart function to a TypeScript file in the project and make sure that the `build` script builds it.
   For example, you can use the example smart function in [Smart functions](/functions/overview) or any of the smart functions in the [examples folder](https://github.com/jstz-dev/jstz/tree/main/examples).

1. Build the smart function with the `build` script, either by running `npm run build` or `yarn build`.
   Jstz builds the smart function with the necessary dependencies and writes the output to `dist/index.ts`.

Now you can deploy the smart function to the local sandbox.
See [Deploying](/functions/deploying).

::: tip

You can verify the size of the built smart function with the `du` command, as in `du -bh dist/index.ts`.
Smart functions must be less than 10MB to be deployed.

:::
