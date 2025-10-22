import * as B from "./module_b.js"

export const function_A1 = () => {
  B.function_B1()
  return "function_A1";
};

export const function_A2 = () => {
  B.function_B2()
  return "function";
};
