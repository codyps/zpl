#![doc = include_str!("../README.md")]

pub mod bitmap_font;
pub mod output;
pub mod parse;
pub mod render;

pub use render::{render, Options};
