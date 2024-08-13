#include "../common/ticket_type.mligo"
#include "../common/jstz_type.mligo"

type normalized_deposit = address * address * (nat * bytes option) * nat

type normalized_fa_deposit =
  address * address option * address * (nat * bytes option) * nat

(* Mocks the jstz smart rollup entrypoint. 
  This contract simply saves all deposit operations in the [normalized_deposit] form. 
*)
module Mock_jstz_smart_rollup = struct
  type storage =
    { native_deposit : normalized_deposit list
    ; fa_deposit : normalized_fa_deposit list
    }

  let normalized_deposit addr ticket : normalized_deposit =
    let (ticketer, (content, amount)), _ = Tezos.read_ticket ticket in
    addr, ticketer, content, amount

  let normalized_fa_deposit addr proxy ticket : normalized_fa_deposit =
    let (ticketer, (content, amount)), _ = Tezos.read_ticket ticket in
    addr, proxy, ticketer, content, amount
	
  [@entry]
  let main (param : jstz) (storage : storage) : operation list * storage =
    match param with
    | Deposit_fa_ticket { receiver; proxy; ticket } ->
      let fa_deposit =
        normalized_fa_deposit receiver proxy ticket :: storage.fa_deposit
      in
      [], { storage with fa_deposit }
    | Deposit_ticket (a, t) ->
      let native_deposit = normalized_deposit a t :: storage.native_deposit in
      [], { storage with native_deposit }
end

let init_jstz_rollup () =
  let storage = { native_deposit = []; fa_deposit = [] } in
  Test.Next.Originate.contract (contract_of Mock_jstz_smart_rollup) storage 0

let assert_failed (result : test_exec_result) (msg : string) =
  match result with
  | Fail _ -> ()
  | Success _ -> failwith msg

let rec assert_lists
    (type a b)
    (assert : a -> b -> unit)
    (actual : a list)
    (expected : b list)
    : unit
  =
  match actual, expected with
  | x :: xs, y :: ys ->
    let () = assert x y in
    assert_lists assert xs ys
  | [], [] -> ()
  | _ -> failwith "Mismatched lengths"
