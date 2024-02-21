mod align;
mod annotate;
mod grep;
mod info;
mod query;
mod tag;
mod to_text;
mod tsv;
mod validate;
mod view;

pub use crate::align::*;
pub use crate::annotate::*;
pub use crate::grep::*;
pub use crate::info::*;
pub use crate::query::*;
pub use crate::tag::*;
pub use crate::to_text::*;
pub use crate::tsv::*;
pub use crate::validate::*;
pub use crate::view::*;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");
