set -x
esbuild index.ts --bundle --format=esm --target=esnext --minify --outfile=dist/index.js
cp echo.js dist/echo.js
