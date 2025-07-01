# Transfer and refund

This example demonstrates how to transfer funds to a smart function and how a smart function transfers back to callers.

There are two smart functions in this example:

- `refund.js`: refunds 1 tez to the caller when the caller transfers 2 tez to the function.
- `transfer.js`: accepts funds from callers, transfers 2 tez to `refund.js`, and refunds 1 tez to callers.

## Setup

1. Deploy `refund.js`.
1. Update the constant `REFUND_ADDRESS` in `transfer.js` with the address of `refund.js`.
1. Deploy `transfer.js`.

## Demo

```
$ jstz account balance -n dev
100ꜩ
$ jstz deploy refund.js -n dev
Smart function deployed by user at address: KT1Ucc8SZpnuQW6R7mpjJqX3fKozbjgYzrgj
Run with `jstz run jstz://KT1Ucc8SZpnuQW6R7mpjJqX3fKozbjgYzrgj/ --network dev`
$ jstz deploy transfer.js -n dev
Smart function deployed by user at address: KT1EmbA8thsZgXqAzd3BGfDmtdM8Fr8zgsSX
Run with `jstz run jstz://KT1EmbA8thsZgXqAzd3BGfDmtdM8Fr8zgsSX/ --network dev`
$ jstz run jstz://KT1EmbA8thsZgXqAzd3BGfDmtdM8Fr8zgsSX/ --network dev -a 2 -t
Connected to trace smart function "KT1EmbA8thsZgXqAzd3BGfDmtdM8Fr8zgsSX"
[INFO]: transferred_amount 2000000

[INFO]: refund_amount 1000000

$ jstz account balance -n dev
99ꜩ
```
