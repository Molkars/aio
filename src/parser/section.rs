use crate::parser::Location;

pub struct Section {
    pub content: String,
    pub location: Location,
    pub length: usize,
}