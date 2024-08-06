(* SPDX-CopyrightText Trilitech <contact@trili.tech> *)
(* SPDX-CopyrightText Nomadic Labs <contact@nomadic-labs.com> *)

(*  XTZ Exchanger
    -------------
    This contract allows users to mint/burn tez tickets in sandbox environment. 
    Implementation of this contract should be identical to the Etherlink exchanger:
    https://gitlab.com/tezos/tezos/-/blob/master/etherlink/tezos_contracts/exchanger.mligo 
*)

#include "./common/ticket_type.mligo"

type storage = unit

type return = operation list * storage

// Mint creates [Tezos.get_amount ()] tickets and transfers them to [address].
[@entry]
let mint address () : return =
  let contract : tez_ticket contract =
    Tezos.get_contract_with_error address "Invalid callback"
  in
  let amount: nat = Tezos.get_amount () / 1mutez in
  let tickets =
    match Tezos.create_ticket (0n, None) amount with
    | Some (t : tez_ticket) -> t
    | None -> failwith "Could not mint ticket."
  in
  ([ Tezos.transaction tickets 0mutez contract ], ())

// Burn destructs the [ticket] and sends back the tez to [address].
[@entry]
let burn (address, (ticket: tez_ticket)) () : return =
  if Tezos.get_amount () > 0tez then
    failwith "Burn does not accept tez."
  else
    let (addr, (_, amt)), _ticket = Tezos.read_ticket ticket in
    if addr <> (Tezos.get_self_address ()) then
      failwith "Burn only accepts tez tickets minted by the exchanger."
    else
      let contract = Tezos.get_contract_with_error address "Invalid callback" in
      let amount : tez = amt * 1mutez in
      ([ Tezos.transaction () amount contract ], ())
