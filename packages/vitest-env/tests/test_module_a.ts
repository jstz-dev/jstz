import { describe, it, expect } from "vitest";
import { function_A1, function_A2 } from "../src/module_a.js";

describe("module A", () => {
  it("function A1 test", () => {
    expect(function_A1(), "function_A1");
  });

  it("function A2 test", () => {
    expect(function_A2(), "function_A2");
  });
});

describe("second module A", () => {
  it("second function A1 test", () => {
    expect(function_A1(), "function_A1");
  });

  it("second function A2 test", () => {
    expect(function_A2(), "function_A2");
  });
});
