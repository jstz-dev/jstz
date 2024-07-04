use crate::sandbox::daemon::SandboxBootstrapAccount;

pub const SANDBOX_LOCAL_HOST_LISTENING_ADDR: &str = "0.0.0.0";
pub const SANDBOX_LOCAL_HOST_ADDR: &str = "127.0.0.1";
pub const SANDBOX_OCTEZ_NODE_PORT: u16 = 18731;
pub const SANDBOX_OCTEZ_NODE_RPC_PORT: u16 = 18730;
pub const SANDBOX_JSTZ_NODE_PORT: u16 = 8933;
pub const SANDBOX_OCTEZ_SMART_ROLLUP_PORT: u16 = 8932;
pub const SANDBOX_BOOTSTRAP_ACCOUNT_XTZ_AMOUNT: u64 = 4000000000000;
pub const SANDBOX_BOOTSTRAP_ACCOUNT_CTEZ_AMOUNT: u64 = 100000000000;
pub const SANDBOX_BOOTSTRAP_ACCOUNTS: [SandboxBootstrapAccount; 5] = [
    SandboxBootstrapAccount {
        address: "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
        secret: "unencrypted:edsk3gUfUPyBSfrS9CCgmCiQsTCHGkviBDusMxDJstFtojtc1zcpsh",
    },
    SandboxBootstrapAccount {
        address: "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN",
        secret: "unencrypted:edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo",
    },
    SandboxBootstrapAccount {
        address: "tz1faswCTDciRzE4oJ9jn2Vm2dvjeyA9fUzU",
        secret: "unencrypted:edsk4ArLQgBTLWG5FJmnGnT689VKoqhXwmDPBuGx3z4cvwU9MmrPZZ",
    },
    SandboxBootstrapAccount {
        address: "tz1b7tUupMgCNw2cCLpKTkSD1NZzB5TkP2sv",
        secret: "unencrypted:edsk2uqQB9AY4FvioK2YMdfmyMrer5R8mGFyuaLLFfSRo8EoyNdht3",
    },
    SandboxBootstrapAccount {
        address: "tz1ddb9NMYHZi5UzPdzTZMYQQZoMub195zgv",
        secret: "unencrypted:edsk4QLrcijEffxV31gGdN2HU7UpyJjA8drFoNcmnB28n89YjPNRFm",
    },
];
