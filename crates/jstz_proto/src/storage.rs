use tezos_smart_rollup::storage::path::RefPath;

pub const ORACLE_PUBLIC_KEY_PATH: RefPath = RefPath::assert_from(b"/oracle/public_key");
pub const ORACLE_REQUESTS_PATH: RefPath = RefPath::assert_from(b"/oracle/requests");
