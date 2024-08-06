(* SPDX-CopyrightText 2023-2024 Trilitech <contact@trili.tech> *)
(* SPDX-CopyrightText Nomadic Labs <contact@nomadic-labs.com> *)

(* Native Token Bridge for Jstz
   ----------------------------
   This contract allows users to deposit tez into Jstz. It is heavily inspired
   the Etherlink bridge contract: 
   https://gitlab.com/tezos/tezos/-/blob/master/etherlink/tezos_contracts/evm_bridge.mligo
*)

#include "./common/ticket_type.mligo"
#include "./common/jstz_type.mligo"

module Tezos = Tezos.Next

type deposit_request = 
  { jstz_address: address (* Jstz rollup address. *)
  ; l2_address: address  
    (* L2 address - supports tz1, tz2, tz3, tz4, kt1 *)
    (* TODO: https://linear.app/tezos/issue/JSTZ-34
       Add check for supported addresses
    *)
  }

type storage = 
  { exchanger: address (* Address of exchanger contract minting tez tickets. *)
  ; deposit_request: deposit_request option; (* Address of L2 depositee *) 
  }

type return = operation list * storage

(* [deposit request {exchanger; deposit_request}] initiates the deposit 
   of ticket tez into the Jstz Smart Rollup at [request.jstz_address], with
   receiver address [request.l2_address]. The function sends tez to [exchanger]
   who will mint tez tickets and invoke the %callback function. %callback will
   then forward the tez tickets to the Jstz Smart Rollup.

   Throws if [deposit_request] is already set. This is a fatal bug.
   Throws if [exchanger] contract does not have %mint entrypoint. This is a fatal bug.
*)
[@entry]
let deposit (request: deposit_request) ({exchanger; deposit_request}: storage) : return =
  let () = 
    match deposit_request with
    | None -> ()
    | Some _ -> failwith "Deposit locked" 
  in
  let amount = Tezos.get_amount () in
  if amount <= 0tez then 
    failwith "Invalid deposit amount: Deposit amount must be greater than 0."
  else
    let callback = Tezos.address ((Tezos.self "%callback") : tez_ticket contract) in
    match Tezos.get_entrypoint_opt "%mint" exchanger with
    | None -> failwith "Invalid tez ticket contract"
    | Some contract ->
      let mint = Tezos.Operation.transaction callback amount contract in
      let callback_storage = { exchanger; deposit_request = Some request } in
      [ mint ], callback_storage

(* [callback ticket {exchanger; deposit_request}] sends a [Deposit_Ticket] 
   payload to [deposit_request.jstz_address], targeting the L2 address 
   given in [deposit_request.l2_address]. 
   
   Throws if [deposit_request] is not set by [deposit] prior to its execution. 
   [deposit_request] is unset at the end of the function.
*)
[@entry]
let callback (ticket: tez_ticket) ({exchanger; deposit_request}: storage) : return =
  let deposit_request =
    match deposit_request with
    | None -> failwith "Callback on non-locked deposit"
    | Some r -> r
  in
  let { jstz_address; l2_address } = deposit_request in
  let jstz_address: jstz contract =
    Tezos.get_contract_with_error jstz_address "Invalid rollup address"
  in
  let deposit =
    Tezos.Operation.transaction
      (Deposit_ticket (l2_address, ticket))
      0mutez
      jstz_address
  in
  let reset_storage = { exchanger; deposit_request = None } in
  [ deposit ], reset_storage
