use std::ops::{Deref, DerefMut};

use bytes::Bytes;
use eyre::eyre;

mod command;
mod format;
mod parse;

pub struct Zpl {}
