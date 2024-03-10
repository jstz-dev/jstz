module Jstz_bridge = struct

  type storage = 
    { rollup : address option
    ; ctez_contract : address
    }

  type deposit = 
    { jstz_address : bytes
    ; amount : nat
    }

  type rollup_type = (bytes * unit ticket)

  type return = operation list * storage

  type fa12_transfer = [@layout comb]
    { [@annot from] from_ : address
    ; [@annot to] to_ : address
    ; value : nat
    }

  [@entry] let deposit (deposit : deposit) (s : storage) : return =
      let from_ = Tezos.get_sender () in
      let self = Tezos.get_self_address () in
      let ctez_contract : fa12_transfer contract =
        Tezos.get_entrypoint_opt "%transfer" s.ctez_contract
        |> Option.value_exn "Expected ctez contract to have entrypoint %transfer"
      in
      let jstz_rollup : rollup_type contract =
        match s.rollup with
        | None -> failwith "jstz rollup address was not set"
        | Some rollup -> Tezos.get_contract_opt rollup |> Option.value_exn "Expected rollup to exist"
      in
      let ticket =
        match Tezos.create_ticket () deposit.amount with
        | Some ticket -> ticket
        | None -> failwith "Amount must be > 0" 
      in
      let jstz_deposit =
        Tezos.transaction (deposit.jstz_address, ticket) 0mutez jstz_rollup 
      in
      let ctez_transfer = 
        Tezos.transaction 
          { from_; to_ = self; value = deposit.amount }
          0mutez 
          ctez_contract
      in
      [ctez_transfer; jstz_deposit], s

  [@entry] let set_rollup (addr : address) (s : storage) : return =
      [], {s with rollup = Some addr}
      
end
