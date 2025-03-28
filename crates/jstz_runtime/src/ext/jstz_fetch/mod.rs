#![allow(unused)]
use std::{cell::RefCell, rc::Rc};

use deno_core::*;
use deno_fetch_ext::FetchHandler;

use crate::Protocol;

#[allow(non_camel_case_types)]
type jstz_fetch = deno_fetch_ext::deno_fetch;
