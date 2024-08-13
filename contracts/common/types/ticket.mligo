(*****************************************************************************)
(*                                                                           *)
(* SPDX-License-Identifier: MIT                                              *)
(* Copyright (c) 2024 Baking Bad                                             *)
(*                                                                           *)
(*****************************************************************************)

#import "../errors.mligo" "Errors"


(*
    Tools for working with tickets
*)
type content_t = nat * bytes option
type t = content_t ticket

let create (content : content_t) (amount : nat) : t =
  match Tezos.create_ticket content amount with
  | None -> failwith Errors.ticket_creation_failed
  | Some t -> t

let get (address : address) : t contract =
  match Tezos.get_contract_opt address with
  | None -> failwith Errors.failed_to_get_ticket_entrypoint
  | Some c -> c

let split (ticket : t) (split_amount : nat) : t * t =
  (* Splits ticket into two tickets with given amounts *)
  let (_, (_, amount)), ticket = Tezos.read_ticket ticket in
  let keep_amount =
    if amount >= split_amount
    then abs (amount - split_amount)
    else failwith Errors.insufficient_amount
  in
  match Tezos.split_ticket ticket (split_amount, keep_amount) with
  | Some split_tickets -> split_tickets
  | None -> failwith Errors.irreducible_amount

let send (ticket : t) (receiver : address) : operation =
  let receiver_contract = get receiver in
  Tezos.transaction ticket 0 receiver_contract
