(* SPDX-CopyrightText 2024 Trilitech <contact@trili.tech> *)

#include "ticket_type.mligo"

type jstz = 
  | Deposit_ticket of (address * tez_ticket)
  | Deposit_fa_ticket of 
    { receiver: address
    ; proxy: address option
    ; ticket: fa_ticket
    }

 