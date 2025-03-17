(*****************************************************************************)
(*                                                                           *)
(* SPDX-License-Identifier: MIT                                              *)
(* Copyright (c) 2024 Baking Bad                                             *)
(*                                                                           *)
(*****************************************************************************)

#import "../errors.mligo" "Errors"


(*
    `ticketer-deposit` is deposit interface for the Ticketer contract
    which represents the amount of token to be wrapped to the ticket
*)
type t = nat

let get (router : address) : t contract =
  match Tezos.get_entrypoint_opt "%deposit" router with
  | None -> failwith Errors.ticketer_deposit_not_found
  | Some entry -> entry

let send (ticketer : address) (amount : t) : operation =
  let entry = get ticketer in
  Tezos.transaction amount 0 entry
