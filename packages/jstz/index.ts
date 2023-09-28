export function isAddress(addr: Address): addr is Address {
  return typeof addr === "string";
}
