//! Stream module contains a set of IO (write/read) wrappers.

pub mod log;
pub mod stdin;

use crate::{Captures, Error, Needle};
use std::io;
