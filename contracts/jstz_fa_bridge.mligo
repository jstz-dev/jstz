#import "./common/entrypoints/ticketer-deposit.mligo" "Ticketer_deposit_entry"
#import "./common/entrypoints/router-withdraw.mligo" "Router_withdraw_entry"
#import "./common/tokens/tokens.mligo" "Token"
#import "./common/types/ticket.mligo" "Ticket"
#import "./common/errors.mligo" "Errors"
#import "./common/assertions.mligo" "Assertions"

#include "./common/jstz_type.mligo"

module Storage = struct
  (* Stores the receiver and rollup address during a bridging process. 
     It is set when deposit entrypoint is executed and cleared after 
     the ticket is received from the ticketer. *)
  type ongoing_deposit =
    { receiver : address (* Address that will receive tokens in Jstz *)
    ; rollup : address (* Address of the Jstz smart rollup contract *)
    }

  type t =
    { token : Token.t (* Token which Ticketer accepts for minting tickets, immutable *)
    ; ticketer : address (* Ticketer address, immutable *)
    ; proxy : address option (* Proxy smart function address, immutable *)
    ; ongoing_deposit : ongoing_deposit option (* Details of the on-going deposit *)
    ; metadata : (string, bytes) big_map
          (* Metadata of the contract (TZIP-016), immutable *)
    }

  let set_ongoing_deposit (ongoing_deposit : ongoing_deposit) (store : t) : t =
    { store with ongoing_deposit = Some ongoing_deposit }

  let clear_ongoing_deposit (store : t) : t = { store with ongoing_deposit = None }
end

module Jstz_fa_bridge = struct
  module Tezos = Tezos.Next

  type storage = Storage.t
  type return = operation list * storage

  type deposit_params =
    { rollup : address (* Jstz rollup address *)
    ; receiver : address (* Address in Jstz that will receive tokens *)
    ; amount : nat (* Amount of tokens to bridge *)
    }

  (* [deposit params s] entrypoint is called when the user wants to bridge tokens
      to Jstz.
      
     This entrypoint will transfer tokens from the user to the Bridge, approve the Ticketer
     to spend [params.amount] tokens on behalf of the Bridge, and then call `Ticketer.deposit` 
     entrypoint, which will transfer tokens from Bridge to the Ticketer, mint a tickets
     and send them back to the bridge triggering the `default` entrypoint. *)
  [@entry]
  let deposit (params : deposit_params) (s : storage) : return =
    let { amount; receiver; rollup } = params in
    let () = Assertions.no_xtz_deposit () in
    let token = s.token in
    let ticketer = s.ticketer in
    let sender = Tezos.get_sender () in
    let self = Tezos.get_self_address () in
    let token_transfer_op = Token.send_transfer token amount sender self in
    let approve_token_op = Token.send_approve token ticketer amount in
    let start_deposit_op = Ticketer_deposit_entry.send ticketer amount in
    let context = { rollup; receiver } in
    let updated_store = Storage.set_ongoing_deposit context s in
    [ token_transfer_op; approve_token_op; start_deposit_op ], updated_store

  (* [default ticket s] entrypoint will receive tickets minted by the Ticketer.
     and forward them to the Jstz rollup stored in [ongoing_deposit.rollup] (in [s]). *)
  [@entry]
  let default (ticket : Ticket.t) (s : storage) : return =
    let () = Assertions.no_xtz_deposit () in
    let () = Assertions.sender_is s.ticketer in
    match s.ongoing_deposit with
    | Some ongoing_deposit ->
      let { rollup; receiver } = ongoing_deposit in
      let jstz : jstz contract =
        Tezos.get_contract_with_error rollup "Invalid rollup address"
      in
      let fa_deposit = Deposit_fa_ticket { receiver; proxy = s.proxy; ticket } in
      let fa_deposit_op = Tezos.Operation.transaction fa_deposit 0 jstz in
      let updated_store = Storage.clear_ongoing_deposit s in
      [ fa_deposit_op ], updated_store
    | None -> failwith Errors.routing_data_is_not_set

  (* [withdraw params s] is added in case the user specific this contract as
      the withdraw target instead of the Ticketer. *)
  [@entry]
  let withdraw (params : Router_withdraw_entry.t) (s : storage) : return =
    let { ticket; receiver } = params in
    let (ticketer, (_, _)), ticket = Tezos.Ticket.read ticket in
    let withdraw = { receiver; ticket } in
    let withdraw_op = Router_withdraw_entry.send ticketer withdraw in
    [ withdraw_op ], s
end
