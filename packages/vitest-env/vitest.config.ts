import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    runner: "./src/test_runner.ts",
    environment: "node",
    include: ["tests/**/*.ts"],
  },
});
