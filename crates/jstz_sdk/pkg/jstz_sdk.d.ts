/* tslint:disable */
/* eslint-disable */
export function sign_operation(operation: any, secret_key: string): string;
export function hash_operation(operation: any): string;
/**
 * Parses signature returned from the passkey device into a valid base58
 * Tezos P256 signature. The passkey signature proivided must be using
 * P256 (alg = -7)
 */
export function parse_passkey_signature(signature: any): string;
