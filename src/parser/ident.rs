use std::borrow::Borrow;
use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use hashbrown::Equivalent;
use crate::parser::Location;

#[derive(Clone)]
pub struct Ident {
    pub value: String,
    pub location: Location,
    pub length: usize,
}

impl Debug for Ident {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.value)
    }
}

impl Display for Ident {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl PartialEq for Ident {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for Ident {}

impl PartialEq<str> for Ident {
    fn eq(&self, other: &str) -> bool {
        self.value == other
    }
}

impl PartialEq<&'_ str> for Ident {
    fn eq(&self, other: &&'_ str) -> bool {
        self.value == *other
    }
}

impl PartialEq<String> for Ident {
    fn eq(&self, other: &String) -> bool {
        self.value == *other
    }
}

impl PartialOrd for Ident {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.partial_cmp(&other.value)
    }
}

impl Ord for Ident {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl Hash for Ident {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl Deref for Ident {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl Borrow<String> for Ident {
    fn borrow(&self) -> &String {
        &self.value
    }
}

impl Borrow<str> for Ident {
    fn borrow(&self) -> &str {
        self.value.as_str()
    }
}

impl AsRef<String> for Ident {
    fn as_ref(&self) -> &String {
        &self.value
    }
}

impl AsRef<str> for Ident {
    fn as_ref(&self) -> &str {
        self.value.as_str()
    }
}

impl AsRef<OsStr> for Ident {
    fn as_ref(&self) -> &OsStr {
        self.value.as_ref()
    }
}

impl Equivalent<str> for Ident {
    fn equivalent(&self, key: &str) -> bool {
        self.value.as_str().equivalent(key)
    }
}

impl Equivalent<String> for Ident {
    fn equivalent(&self, key: &String) -> bool {
        self.value.equivalent(key)
    }
}