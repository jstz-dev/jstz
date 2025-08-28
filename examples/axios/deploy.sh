echo "Deploying dist/echo.js"
jstz deploy dist/echo.js -n dev
echo
echo "Deploying dist/index.js with name axios.example"
jstz deploy dist/index.js --name axios.example -n dev --force
