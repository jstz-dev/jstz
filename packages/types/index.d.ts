declare type Address = string;

declare interface Kv {
  get<T = unknown>(key: string): T | null;
  set(key: string, value: unknown): void;
  delete(key: string): void;
  contains(key: string): boolean;
}

declare var Kv: Kv;

declare type Mutez = number;

declare interface Ledger {
  readonly selfAddress: Address;
  balance(address: Address): Mutez;
  transfer(address: Address, amount: Mutez): void;
}

declare var Ledger: Ledger;
