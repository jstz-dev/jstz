(*****************************************************************************)
(*                                                                           *)
(* SPDX-License-Identifier: MIT                                              *)
(* Copyright (c) 2024 Baking Bad                                             *)
(*                                                                           *)
(*****************************************************************************)

#import "./errors.mligo" "Errors"

let address_is_self (address : address) : unit =
  if address <> Tezos.get_self_address ()
  then failwith Errors.unauthorized_ticketer
  else unit

let no_xtz_deposit (unit : unit) : unit =
  if Tezos.get_amount () > 0mutez then failwith Errors.xtz_deposit_disallowed else unit

let sender_is (address : address) : unit =
  if address <> Tezos.get_sender () then failwith Errors.unexpected_sender else unit
