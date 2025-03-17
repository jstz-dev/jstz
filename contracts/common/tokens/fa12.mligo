(* SPDX-License-Identifier: MIT *)  
(* Copyright (c) 2024 Baking Bad *)

#import "../errors.mligo" "Errors"

type t = address

type transfer_params =
  { [@annot:from] from_ : address
  ; [@annot:to]  to_ : address
  ; value : nat
  }

let get_transfer (token_address : address) : transfer_params contract =
  match Tezos.get_entrypoint_opt "%transfer" token_address with
  | None -> failwith Errors.invalid_fa12
  | Some entry -> entry

let send_transfer
    (from_ : address)
    (to_ : address)
    (token_address : address)
    (value : nat)
    : operation
  =
  let params = { from_; to_; value } in
  let entry = get_transfer token_address in
  Tezos.transaction params 0 entry

type approve_params =
  { spender : address
  ; value : nat
  }

let get_approve (contract_address : address) : approve_params contract =
  match Tezos.get_entrypoint_opt "%approve" contract_address with
  | None -> failwith Errors.invalid_fa12
  | Some entry -> entry

let send_approve (contract_address : address) (spender : address) (value : nat)
    : operation
  =
  let params = { spender; value } in
  let entry = get_approve contract_address in
  Tezos.transaction params 0 entry
