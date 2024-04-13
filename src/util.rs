use std::hash::{Hash, Hasher};
use ahash::AHasher;

#[inline]
pub fn hash<T: Hash>(value: &T) -> u64 {
    let mut hasher = AHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}

pub trait CacheHash {
    fn hash<H: Hasher>(&self, hasher: &mut H);
}

#[inline]
pub fn cache_hash<T: CacheHash>(value: &T) -> u64 {
    let mut hasher = AHasher::default();
    value.hash(&mut hasher);
    hasher.finish()
}