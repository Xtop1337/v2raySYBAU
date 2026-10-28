mod build;
mod parse;

pub use build::build_link;
pub use parse::{is_share_link, parse_link, ParseError};

#[cfg(test)]
mod tests;
