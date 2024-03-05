/*
    STAM Library (Stand-off Text Annotation Model)
        by Maarten van Gompel <proycon@anaproy.nl>
        Digital Infrastucture, KNAW Humanities Cluster

        Licensed under the GNU General Public License v3

        https://github.com/annotation/stam-tools
*/

//! This library powers the command line tools that offer various functionality for STAM.

pub mod align;
pub mod annotate;
pub mod grep;
pub mod info;
pub mod query;
pub mod tag;
pub mod to_text;
pub mod transpose;
pub mod tsv;
pub mod validate;
pub mod view;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");
