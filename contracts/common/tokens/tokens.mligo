(*****************************************************************************)
(*                                                                           *)
(* SPDX-License-Identifier: MIT                                              *)
(* Copyright (c) 2024 Baking Bad                                             *)
(*                                                                           *)
(*****************************************************************************)

#import "./fa2.mligo" "TokenFa2"
#import "./fa12.mligo" "TokenFa12"
#import "../errors.mligo" "Errors"

type token_info_t = (string, bytes) map

type t =
  | Fa12 of TokenFa12.t
  | Fa2 of TokenFa2.t

let send_transfer (token : t) (amount : nat) (from_ : address) (to_ : address) : operation
  =
  match token with
  | Fa12 addr -> TokenFa12.send_transfer from_ to_ addr amount
  | Fa2 (addr, token_id) ->
    let txs = [ { to_; token_id; amount } ] in
    TokenFa2.send_transfer from_ addr txs

let send_approve (token : t) (operator : address) (amount : nat) : operation =
  match token with
  | Fa12 contract_address -> TokenFa12.send_approve contract_address operator amount
  | Fa2 (contract_address, token_id) ->
    TokenFa2.send_approve contract_address token_id operator
