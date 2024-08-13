(*
  Subject   : Jstz Native Bridge contract test
  Invocation: ligo run test contracts/test/test_jstz_native_bridge.mligo --no-warn
*)

#import "../jstz_native_bridge.mligo" "Jstz_native_bridge"
#import "../exchanger.mligo" "Exchanger"

#include "../common/ticket_type.mligo"
#include "../common/jstz_type.mligo"
#include "./helpers.mligo"

(* Sets up the exchanger, bridge, mock jstz rollup, l2 address and deposit request *)
let setup_contracts_and_request () = 
  let exchanger = 
    let exchanger_contract = 
      Test.Next.Originate.contract 
        (contract_of Exchanger) 
        ()
        0tez 
    in
    Test.Next.Typed_address.to_address exchanger_contract.taddr
  in
  let bridge = 
    Test.Next.Originate.contract 
      (contract_of Jstz_native_bridge) 
      { exchanger; deposit_request = None} 
      0tez 
  in
  let jstz_rollup = init_jstz_rollup () in
  let l2_address = Test.Next.Account.bob () in
  let deposit_request = {
    jstz_address = (Test.Next.Typed_address.to_address jstz_rollup.taddr);
    l2_address;
  } 
  in
  { exchanger; bridge; jstz_rollup; l2_address; deposit_request }

let test_deposit_successful = 
  let { exchanger; bridge; jstz_rollup; l2_address; deposit_request } = setup_contracts_and_request () in
  let _ = Test.Next.Typed_address.transfer_exn bridge.taddr (Deposit deposit_request) 100tez in
  let expected: normalized_deposit list = [ (l2_address, exchanger, (0n, None), 100000000n) ] in
  let { native_deposit = actual; fa_deposit = _; } = 
    Test.Next.get_storage jstz_rollup.taddr in
  let () =  assert_lists (fun x y -> assert (Test.equal x y)) expected actual in
  let bridge_storage = Test.Next.get_storage bridge.taddr in
  assert (Test.equal bridge_storage { exchanger; deposit_request = None })

let test_deposit_deposit_request_locked_throws_error = 
  let { exchanger; bridge = _; jstz_rollup = _; l2_address = _; deposit_request } = setup_contracts_and_request () in
  let bridge = 
    // Override the bridge initial storage
    Test.Next.Originate.contract 
      (contract_of Jstz_native_bridge) 
      { exchanger; deposit_request = Some deposit_request} 
      0tez
  in 
  let result = Test.Next.Typed_address.transfer bridge.taddr (Deposit deposit_request) 100tez in
  assert_failed result "Expected error when deposit request is locked"

let test_callback_direct_call_throws_error = 
  let { exchanger = _; bridge; jstz_rollup = _; l2_address = _; deposit_request = _ } = setup_contracts_and_request () in
  let ticket : tez_ticket = 
    Option.value_exn 
      "Failed to create ticket" 
      (Tezos.Next.Ticket.create (0n, None) 100n) 
  in
  let result = Test.Next.Typed_address.transfer bridge.taddr (Callback ticket) 0tez in
  assert_failed result "Expected error when callback is called directly"

