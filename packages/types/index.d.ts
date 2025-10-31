/**
 * Jstz account address.
 */
declare type Address = string;

/**
 * The interface for manipulating smart function data store.
 * 
 * Jstz smart functions store data in a persistent key-value database. This database is built
 * directly into the Jstz runtime, available using the global Kv object.
 */
declare interface Kv {
  /**
   * Retrieve the value for the given key from the database.
   * 
   * @param key Key of interest.
   * @returns The value held by the designated key. If no value exists for the key,
   * `null` is returned.
   */
  get<T = unknown>(key: string): T | null;
  /**
   * Set the value for the given key in the database. If a value already exists for the key,
   * it will be overwritten.
   * 
   * @param key Key to hold the value.
   * @param value Value to be stored.
   */
  set(key: string, value: unknown): void;
  /**
   * Deletes the value for the given key from the database. If no value exists for the key,
   * this function is a no-op.
   * 
   * @param key Key that holds the data to be deleted.
   */
  delete(key: string): void;
  /**
   * Checks if a value exists for the given key in the database.
   * 
   * @param key Key of interest.
   * @returns `true` if a value exists for the given key in the database, `false` otherwise.
   */
  contains(key: string): boolean;
}

/**
 * The interface for manipulating smart function data store.
 * 
 * Jstz smart functions store data in a persistent key-value database. This database is built
 * directly into the Jstz runtime, available using the global `Kv` object.
 */
declare var Kv: Kv;

/**
 * The basic unit of the native cryptocurrency of Tezos, the tez. One mutez is equal to one millionth of a tez.
 */
declare type Mutez = number;

/**
 * The `Ledger` object maintains a persistent ledger of all accounts and their balances of L2
 * tez (stored as mutez). Additionally, the `Ledger` object stores the 'self address' of
 * the smart function, which is the address of the smart function itself.
 * 
 * All operations on `Ledger` are synchronous and atomic, committed if the request to the smart function succeeds.
 * 
 * @deprecated The Ledger API is deprecated and will be removed in future versions of Jstz.
 */
declare interface Ledger {
  /**
   * The `selfAddress` property of the `Ledger` object is the address of the smart function.
   */
  readonly selfAddress: Address;
  /**
   * Retrieves the balance of the given address in mutez, or `0` if the address is not in the ledger.
   * 
   * @param address Address of interest.
   * @returns The balance of the designated address in mutez.
   */
  balance(address: Address): Mutez;
  /**
   * Transfers the given amount of mutez from the balance of the smart function to the given address.
   * 
   * @param address Payee address.
   * @param amount Amount to be transferred in mutez.
   * @throws `RuntimeError` Thrown when the smart function does not have enough balance.
   */
  transfer(address: Address, amount: Mutez): void;
}

/**
 * The `Ledger` object maintains a persistent ledger of all accounts and their balances of L2
 * tez (stored as mutez). Additionally, the `Ledger` object stores the 'self address' of
 * the smart function, which is the address of the smart function itself.
 * 
 * All operations on `Ledger` are synchronous and atomic, committed if the request to the smart function succeeds.
 * 
 * @deprecated The Ledger API is deprecated and will be removed in future versions of Jstz.
 */
declare var Ledger: Ledger;
