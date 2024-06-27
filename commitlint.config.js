module.exports = {
  extends: ["@commitlint/config-conventional"],
  rules: {
    // Ensure the body does not exceed 200 characters (default of config-conventional is 100)
    "body-max-line-length": [2, "always", 200],
  },
};
