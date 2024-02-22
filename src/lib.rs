pub mod align;
pub mod annotate;
pub mod grep;
pub mod info;
pub mod query;
pub mod tag;
pub mod to_text;
pub mod tsv;
pub mod validate;
pub mod view;

/*
pub use align::{
    align, align_arguments, align_texts, alignments_tsv_out, AlignmentAlgorithm, AlignmentConfig,
    AlignmentScope,
};
pub use annotate::*;
pub use grep::*;
pub use info::*;
pub use query::*;
pub use tag::*;
pub use to_text::*;
pub use tsv::*;
pub use validate::*;
pub use view::*;
*/

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");
