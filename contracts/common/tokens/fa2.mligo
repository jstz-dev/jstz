(*****************************************************************************)
(*                                                                           *)
(* SPDX-License-Identifier: MIT                                              *)
(* Copyright (c) 2024 Baking Bad                                             *)
(*                                                                           *)
(*****************************************************************************)

#import "../errors.mligo" "Errors"

type t = address * nat

type transfer_txs_item =
  { to_ : address
  ; token_id : nat
  ; amount : nat
  }

type transfer_txs = transfer_txs_item list

type transfer_params =
  { from_ : address
  ; txs : transfer_txs
  } list

let get_transfer (address : address) : transfer_params contract =
  match Tezos.get_entrypoint_opt "%transfer" address with
  | None -> failwith Errors.invalid_fa2
  | Some entry -> entry

let send_transfer (from_ : address) (token_address : address) (txs : transfer_txs)
    : operation
  =
  let params = [ { from_; txs } ] in
  let entry = get_transfer token_address in
  Tezos.transaction params 0 entry

type operator_param_t =
  { owner : address
  ; operator : address
  ; token_id : nat
  }

type update_operator_param_t =
  | Add_operator of operator_param_t
  | Remove_operator of operator_param_t

type update_operator_params_t = update_operator_param_t list

let get_approve (address : address) : update_operator_params_t contract =
  match Tezos.get_entrypoint_opt "%update_operators" address with
  | None -> failwith Errors.invalid_fa2
  | Some entry -> entry

let send_approve (contract_address : address) (token_id : nat) (operator : address)
    : operation
  =
  let owner = Tezos.get_self_address () in
  let operator_param = { operator; token_id; owner } in
  let params = [ Add_operator operator_param ] in
  let entry = get_approve contract_address in
  Tezos.transaction params 0 entry
