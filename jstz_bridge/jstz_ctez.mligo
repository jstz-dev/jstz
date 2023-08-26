// Sandbox ctez contract
// This is a partial implementation of a FA1.2 contract that
// supports the functionality expected from ctez for deposits
// 
// `mint` entrypoint added to change balances in the sandbox

module Balance = struct

  type t = nat
  type delta = int

  let add (t : t) (u : t) : t = t + u

  let add_delta (t : t) (delta : delta) : t = 
    match is_nat (t + delta) with
    | None -> failwith "Balance underflow"
    | Some t -> t


  let sub (t : t) (u : t) : t = 
    match is_nat (t - u) with
    | None -> failwith "Balance underflow"
    | Some t -> t

end

module Jstz_ctez = struct

  type storage = 
    { tokens: (address, nat) big_map
    ; admin: address
    }

  type return = operation list * storage

  type transfer = [@layout comb]
    { [@annot from] from_ : address
    ; [@annot to] to_ : address
    ; value : nat
    }

  type mint = [@layout comb]
    { quantity: int
    ; target: address
    }
  
  let get_balance (s : storage) address = 
    match Big_map.find_opt address s.tokens with
    | None -> 0n
    | Some balance -> balance

  let set_balance (s : storage) address balance = 
    if balance = 0n
    then s
    else 
      let tokens = Big_map.update address (Some balance) s.tokens in
      { s with tokens }

  let update_balance (s : storage) address f = 
    set_balance s address (f (get_balance s address))

  [@entry] let mint (mint : mint) (s : storage) : return = 
    if Tezos.get_sender () <> s.admin
    then failwith "Only `admin` can mint tokens"
    else
      let s =
        let balance = get_balance s mint.target in
        set_balance s mint.target (Balance.add_delta balance mint.quantity)
      in 
      [], s

  [@entry] let transfer (transfer : transfer) (s : storage) : return = 
    let s = 
      let from_balance = get_balance s transfer.from_ in
      set_balance s transfer.from_ (Balance.sub from_balance transfer.value)
    in
    let s =
      let to_balance = get_balance s transfer.to_ in
      set_balance s transfer.to_ (Balance.add to_balance transfer.value)
    in
    [], s

end