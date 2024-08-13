(*
  Subject   : Jstz FA Bridge contract test
  Invocation: ligo run test --no-warn contracts/test/test_jstz_fa_bridge.mligo
*)

#include "../jstz_fa_bridge.mligo" 
#import "../examples/fa_ticketer/fa_ticketer.mligo" "Ticketer"
#import "../common/tokens/fa2.mligo" "Fa2"
#import "../common/tokens/fa12.mligo" "Fa12"
#import "../common/tokens/tokens.mligo" "Token"

#include "../common/ticket_type.mligo"
#include "../common/jstz_type.mligo"
#include "./helpers.mligo"

module Mock_fa2_token = struct
  type call = 
    | Transfer_fa2 of Fa2.transfer_params
    | Update_operators of Fa2.update_operator_params_t

  type storage = call list
  
  type return = operation list * storage
  
  [@entry]
  let transfer (p: Fa2.transfer_params) (s: storage): return =
    [], Transfer_fa2 p :: s

  [@entry]
  let update_operators (p: Fa2.update_operator_params_t) (s: storage): return =
    [], Update_operators p :: s

  let assert_call x y = 
    match (x, y) with
    | Transfer_fa2 x, Transfer_fa2 y -> 
        assert_lists (fun a b -> 
          let { from_ = a_from; txs = a_txs } = a in
          let { from_ = b_from; txs = b_txs } = b in
          let () = assert_with_error (Test.equal a_from b_from) "Mismatched fa2 from_" in
          assert_lists (fun m n -> assert_with_error (Test.equal m n) "Mismatched fa2 txn") a_txs b_txs
        ) x y 
    | Update_operators x, Update_operators y -> assert_lists (fun a b -> assert (Test.equal a b)) x y 
    | _ -> failwith "Expected same calls"
end

module Mock_fa12_token = struct 
  type call = 
    | Transfer_fa12 of Fa12.transfer_params
    | Approve of Fa12.approve_params

  type storage = call list
  
  type return = operation list * storage
  
  [@entry]
  let transfer (p: Fa12.transfer_params) (s: storage): return =
    [], Transfer_fa12 p :: s

  [@entry]
  let approve(p: Fa12.approve_params) (s: storage): return =
    [], Approve p :: s

  let assert_call x y = 
    match (x, y) with
    | Transfer_fa12 x, Transfer_fa12 y -> assert (Test.equal x y)
    | Approve x, Approve y -> assert (Test.equal x y)
    | _ -> failwith "Expected same calls"
end

module No_transfer_fa2 = struct
  type return = operation list * unit

  [@entry]
  let not_transfer (_: Fa2.transfer_params) (): return =
    [], ()

  [@entry]
  let update_operators (_: Fa2.update_operator_params_t) (): return =
    [], ()
end

module No_update_operators_fa2 = struct
  type return = operation list * unit

  [@entry]
  let transfer (_ : Fa2.transfer_params) () : return = [], ()

  [@entry]
  let not_update_operators (_ : Fa2.update_operator_params_t) () : return = [], ()
end

module No_transfer_fa12 = struct
  type return = operation list * unit

  [@entry]
  let not_transfer (_ : Fa12.transfer_params) () : return = [], ()

  [@entry]
  let approve (_ : Fa12.approve_params) () : return = [], ()
  end

module No_approve_fa12 = struct
  type return = operation list * unit

  [@entry]
  let transfer (_ : Fa12.transfer_params) () : return = [], ()

  [@entry]
  let not_approve (_ : Fa12.approve_params) () : return =
    [], ()
end

module No_mint_ticketer = struct
  [@entry]
  let not_mint (_amount : nat) () : operation list * unit = [], ()
end

let setup token ticketer_content = 
  let ticketer = 
    let storage: Ticketer.Ticketer.storage = { 
      metadata = Big_map.empty; 
      token; 
      content = ticketer_content; 
      total_supply = 0 
    } 
    in
    Test.Next.Originate.contract 
      (contract_of Ticketer.Ticketer) 
      storage
      0tez
  in
  let fa_bridge = 
    let storage: Jstz_fa_bridge.storage  = { 
      token; 
      ticketer = Test.Next.Typed_address.to_address ticketer.taddr; 
      proxy = None; 
      metadata = Big_map.empty; 
      ongoing_deposit = None 
    } 
    in
    Test.Next.Originate.contract 
      (contract_of Jstz_fa_bridge) 
      storage 
      0tez
  in
  let jstz_rollup = init_jstz_rollup () in
  let deposit_request: Jstz_fa_bridge.deposit_params = {
    rollup = Test.Next.Typed_address.to_address jstz_rollup.taddr;
    receiver = Test.Next.Account.bob();
    amount = 100;
  }
  in
  { 
    content = ticketer_content; 
    ticketer = ticketer.taddr; 
    fa_bridge; 
    jstz_rollup; 
    deposit_request
  }

let setup_fa2 () = 
  let fa2_token = 
    Test.Next.Originate.contract 
      (contract_of Mock_fa2_token) 
      []
      0mutez
  in
  let fa2_token_addr = Test.Next.Typed_address.to_address fa2_token.taddr in
  let token_id = 7n in
  let token = Fa2 (fa2_token_addr, token_id) in
  let content: Ticket.content_t = (token_id , Some (Bytes.pack fa2_token_addr))  in
  let { content; ticketer; fa_bridge; jstz_rollup; deposit_request } = 
    setup token content
  in
  { 
    fa2_token; 
    token_id; 
    content; 
    ticketer; 
    fa_bridge; 
    jstz_rollup; 
    deposit_request
  }

let setup_fa12 () = 
  let fa12_token = 
    Test.Next.Originate.contract 
      (contract_of Mock_fa12_token) 
      []
      0mutez
  in
  let fa12_token_addr = Test.Next.Typed_address.to_address fa12_token.taddr in
  let token = Fa12 fa12_token_addr in
  let content: Ticket.content_t = (10n , Some (Bytes.pack fa12_token_addr))  in
  let { content; ticketer; fa_bridge; jstz_rollup; deposit_request } = 
    setup token content
  in 
  {
    fa12_token; 
    content; 
    ticketer; 
    fa_bridge; 
    jstz_rollup; 
    deposit_request
  }

let test_successful_fa2_deposit = 
  let ctx = setup_fa2 () in
  let deposit_entrypoint = 
    Tezos.get_entrypoint 
      "%deposit" 
      (Test.Next.Typed_address.to_address ctx.fa_bridge.taddr) 
  in
  let sender = Test.Next.Account.alice () in
  let () = Test.set_source sender in
  let _ = 
    Test.Next.Contract.transfer_exn 
      deposit_entrypoint
      ctx.deposit_request
      0tez 
  in
  let { native_deposit = _; fa_deposit = actual; } = 
    Test.Next.get_storage ctx.jstz_rollup.taddr 
  in
  let ticketer_addr = Test.Next.Typed_address.to_address ctx.ticketer in
  let () = assert_lists 
    (fun x y -> assert (Test.equal x y)) 
    [(
      ctx.deposit_request.receiver, 
      None, 
      ticketer_addr, 
      ctx.content,
      100n
    )] 
    actual
  in
  let calls: Mock_fa2_token.storage = 
    Test.Next.get_storage ctx.fa2_token.taddr 
  in
  let expected: Mock_fa2_token.storage = [
      Transfer_fa2 [{
          from_ = Test.Next.Typed_address.to_address ctx.fa_bridge.taddr;
          txs = [{
            to_ = ticketer_addr;
            token_id = ctx.token_id;
            amount = 100n;
          }]
      }];
      Update_operators [ 
        Add_operator {
          owner = Test.Next.Typed_address.to_address ctx.fa_bridge.taddr;
          operator = Test.Next.Typed_address.to_address ctx.ticketer;
          token_id = ctx.token_id;
        }
      ];
      Transfer_fa2 [{
        from_ = sender;
        txs = [{
          to_ = Test.Next.Typed_address.to_address ctx.fa_bridge.taddr; // should be bridge
          token_id = ctx.token_id;
          amount = 100n;
        }]
      }];
    ]
  in
  assert_lists 
    Mock_fa2_token.assert_call
    expected
    calls

let test_successful_fa12_deposit =
  let ctx = setup_fa12 () in
  let deposit_entrypoint =
    Tezos.get_entrypoint
      "%deposit"
      (Test.Next.Typed_address.to_address ctx.fa_bridge.taddr) in
  let sender = Test.Next.Account.alice () in
  let _ = Test.set_source sender in
  let _ =
    Test.Next.Contract.transfer_exn 
      deposit_entrypoint 
      ctx.deposit_request 
      0tez 
  in
  let {
   native_deposit = _;
   fa_deposit = actual
  } = Test.Next.get_storage ctx.jstz_rollup.taddr in
  let () = assert_lists
    (fun x y -> assert (Test.equal x y))
    [
      (ctx.deposit_request.receiver,
       None,
       (Test.Next.Typed_address.to_address ctx.ticketer),
       ctx.content,
       100n)
    ]
    actual
  in
  let ticketer_addr = Test.Next.Typed_address.to_address ctx.ticketer in
  let calls: Mock_fa12_token.storage = 
    Test.Next.get_storage ctx.fa12_token.taddr
  in
  let expected = [
    Transfer_fa12 {
      from_ = Test.Next.Typed_address.to_address ctx.fa_bridge.taddr;
      to_ = ticketer_addr;
      value = 100n;
    };
    Approve {
      spender = ticketer_addr;
      value = 100n;
    };
    Transfer_fa12 {
      from_ = sender;
      to_ = Test.Next.Typed_address.to_address ctx.fa_bridge.taddr;
      value = 100n;
    };
  ]
  in
  assert_lists 
    Mock_fa12_token.assert_call
    expected
    calls

let test_fa2_deposit_fails_when_no_transfer_entrypoint = 
  let fa2_token = 
    Test.Next.Originate.contract 
      (contract_of No_transfer_fa2) 
      ()
      0mutez
  in
  let fa2_token_addr = Test.Next.Typed_address.to_address fa2_token.taddr in
  let token_id = 7n in
  let token = Fa2 (fa2_token_addr, token_id) in
  let content: Ticket.content_t = (token_id , Some (Bytes.pack fa2_token_addr))  in
  let ctx = setup token content in
  let deposit_entrypoint =
    Tezos.get_entrypoint
      "%deposit"
      (Test.Next.Typed_address.to_address ctx.fa_bridge.taddr)
  in
  let result = 
    Test.Next.Contract.transfer
      deposit_entrypoint
      ctx.deposit_request
      0tez 
  in
  assert_failed 
    result 
    "Expected error when transfer entrypoint not found on Fa2 token"

let test_fa2_deposit_fails_when_no_update_operators_entrypoint =
  let fa2_token =
    Test.Next.Originate.contract (contract_of No_update_operators_fa2) () 0mutez in
  let fa2_token_addr = Test.Next.Typed_address.to_address fa2_token.taddr in
  let token_id = 7n in
  let token = Fa2 (fa2_token_addr, token_id) in
  let content : Ticket.content_t =
    (token_id, Some (Bytes.pack fa2_token_addr)) in
  let ctx = setup token content in
  let deposit_entrypoint =
    Tezos.get_entrypoint
      "%deposit"
      (Test.Next.Typed_address.to_address ctx.fa_bridge.taddr) in
  let result =
    Test.Next.Contract.transfer deposit_entrypoint ctx.deposit_request 0tez in
  assert_failed
    result
    "Expected error when update operators entrypoint not found on Fa2 token"

let test_fa12_deposit_fails_when_no_transfer_entrypoint =
  let fa12_token =
    Test.Next.Originate.contract (contract_of No_transfer_fa12) () 0mutez in
  let fa12_token_addr = Test.Next.Typed_address.to_address fa12_token.taddr in
  let token = Fa12 fa12_token_addr in
  let content : Ticket.content_t = (0n, Some (Bytes.pack fa12_token_addr)) in
  let ctx = setup token content in
  let deposit_entrypoint =
    Tezos.get_entrypoint
      "%deposit"
      (Test.Next.Typed_address.to_address ctx.fa_bridge.taddr) in
  let result =
    Test.Next.Contract.transfer deposit_entrypoint ctx.deposit_request 0tez in
  assert_failed
    result
    "Expected error when transfer entrypoint not found on Fa12 token"

let test_fa12_deposit_fails_when_no_transfer_entrypoint =
  let fa12_token =
    Test.Next.Originate.contract (contract_of No_approve_fa12) () 0mutez in
  let fa12_token_addr = Test.Next.Typed_address.to_address fa12_token.taddr in
  let token = Fa12 fa12_token_addr in
  let content : Ticket.content_t = (0n, Some (Bytes.pack fa12_token_addr)) in
  let ctx = setup token content in
  let deposit_entrypoint =
    Tezos.get_entrypoint
      "%deposit"
      (Test.Next.Typed_address.to_address ctx.fa_bridge.taddr) in
  let result =
    Test.Next.Contract.transfer deposit_entrypoint ctx.deposit_request 0tez in
  assert_failed
    result
    "Expected error when approve entrypoint not found on Fa12 token"

let test_deposit_fails_when_ticketer_not_found = 
  let ctx = setup_fa2 () in
  let fa_bridge = 
    let storage: Jstz_fa_bridge.storage  = { 
      token = Fa2 (Test.Next.Typed_address.to_address ctx.fa2_token.taddr, 7n); 
      ticketer = "KT1TxqZ8QtKvLu3V3JH7Gx58n7Co8pgtpQU5";
      proxy = None; 
      metadata = Big_map.empty; 
      ongoing_deposit = None 
    } 
    in
    Test.Next.Originate.contract 
      (contract_of Jstz_fa_bridge) 
      storage 
      0tez
  in
  let deposit_entrypoint = 
    Tezos.get_entrypoint 
      "%deposit" 
      (Test.Next.Typed_address.to_address fa_bridge.taddr) 
  in
  let result = 
    Test.Next.Contract.transfer
      deposit_entrypoint
      ctx.deposit_request
      0tez 
  in
  assert_failed 
    result 
    "Expected error when ticketer not found"

let test_deposit_fails_when_ticketer_mint_entrypoint_not_found =
  let ctx = setup_fa2 () in
  let ticketer = 
    Test.Next.Originate.contract 
      (contract_of No_mint_ticketer) 
      ()
      0tez
  in
  let fa_bridge = 
    let storage: Jstz_fa_bridge.storage  = { 
      token = Fa2 (Test.Next.Typed_address.to_address ctx.fa2_token.taddr, 7n); 
      ticketer = Test.Next.Typed_address.to_address ticketer.taddr;
      proxy = None; 
      metadata = Big_map.empty; 
      ongoing_deposit = None 
    } 
    in
    Test.Next.Originate.contract 
      (contract_of Jstz_fa_bridge) 
      storage 
      0tez
  in
  let deposit_entrypoint = 
    Tezos.get_entrypoint 
      "%deposit" 
      (Test.Next.Typed_address.to_address fa_bridge.taddr) 
  in
  let result = 
    Test.Next.Contract.transfer
      deposit_entrypoint
      ctx.deposit_request
      0tez 
  in
  assert_failed 
    result 
    "Expected error when ticketer does not have mint entrypoint"


let test_deposit_fails_when_receives_tez = 
  let ctx = setup_fa2 () in
  let deposit_entrypoint = 
    Tezos.get_entrypoint 
      "%deposit" 
      (Test.Next.Typed_address.to_address ctx.fa_bridge.taddr) 
  in
  let result = 
    Test.Next.Contract.transfer 
      deposit_entrypoint
      ctx.deposit_request
      1mutez 
  in
  assert_failed 
    result 
    "Expected error when deposit receives tez"


let test_deposit_fails_when_callback_invoked_directly = 
  let ctx = setup_fa2 () in
  let callback_entrypoint = 
    Tezos.get_contract (Test.Next.Typed_address.to_address ctx.fa_bridge.taddr) 
  in
  let ticket : tez_ticket = 
    Option.value_exn 
      "Failed to create ticket" 
      (Tezos.Next.Ticket.create (7n, None) 100n) 
  in
  let result = 
    Test.Next.Contract.transfer 
      callback_entrypoint
       ticket 0tez 
  in
  assert_failed 
    result 
    "Expected error when callback invoked directly"
