(*****************************************************************************)
(*                                                                           *)
(* SPDX-License-Identifier: MIT                                              *)
(* Copyright (c) 2024 Baking Bad                                             *)
(*                                                                           *)
(*****************************************************************************)

#import "../../common/tokens/tokens.mligo" "Token"
#import "../../common/types/ticket.mligo" "Ticket"
#import "../../common/entrypoints/ticketer-deposit.mligo" "TicketerDepositEntry"
#import "../../common/entrypoints/router-withdraw.mligo" "RouterWithdrawEntry"
#import "../../common/errors.mligo" "Errors"
#import "../../common/assertions.mligo" "Assertions"
#import "./storage.mligo" "Storage"

module Ticketer = struct
  (*
        Ticketer is a contract that allows to convert any legacy FA1.2 or
        FA2 token to a ticket. The legacy token can be later released by the
        ticket holder.

        Only one token supported per ticketer contract.

        Information about the token added to the ticket as FA2.1 compatible
        payload `(pair nat (option bytes))` where bytes is a packed
        `token_info` record provided during the ticketer origination.
    *)
  type storage = Storage.t
  type return_t = operation list * storage

  let assert_content_is_expected
      (content : Ticket.content_t)
      (expected : Ticket.content_t)
      : unit
    =
    assert_with_error (content = expected) Errors.unexpected_content
  
  [@entry]
  let deposit (amount : TicketerDepositEntry.t) (store : storage) : return_t =
    (*
            `deposit` entrypoint is used to convert legacy token to a ticket.
            The legacy token is transferred to the ticketer contract and
            the ticket is minted.

            @param amount: amount of the token to be converted to the ticket.
        *)
    let () = Assertions.no_xtz_deposit () in
    let store = Storage.increase_total_supply amount store in
    let self = Tezos.get_self_address () in
    let sender = Tezos.get_sender () in
    let ticket = Ticket.create store.content amount in
    let token_transfer_op = Token.send_transfer store.token amount sender self in
    let ticket_transfer_op = Ticket.send ticket sender in
    [ token_transfer_op; ticket_transfer_op ], store

  [@entry]
  let withdraw (params : RouterWithdrawEntry.t) (store : Storage.t) : return_t =
    (*
            `withdraw` entrypoint is used to release the legacy token from the
            ticket. The ticket is burned and the legacy token is transferred
            to the ticket holder.

            @param receiver: an address which will receive the unlocked token.
            @param ticket: provided ticket to be burned.
        *)
    let { ticket; receiver } = params in
    let (ticketer, (content, amount)), _ = Tezos.read_ticket ticket in
    let () = assert_content_is_expected content store.content in
    let () = Assertions.address_is_self ticketer in
    let () = Assertions.no_xtz_deposit () in
    let store = Storage.decrease_total_supply amount store in
    let transfer_op = Token.send_transfer store.token amount ticketer receiver in
    [ transfer_op ], store

  [@view]
  let get_total_supply (() : unit) (store : storage) : nat = store.total_supply

  [@view]
  let get_content (() : unit) (store : storage) : Ticket.content_t = store.content

  [@view]
  let get_token (() : unit) (store : storage) : Token.t = store.token
end


