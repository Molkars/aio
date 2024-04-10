use std::rc::Rc;
use hashbrown::HashMap;

pub struct TypeStore {
    inner: HashMap<String, Rc<dyn Type>>,
}

impl Default for TypeStore {
    fn default() -> Self {
        let mut out = Self::empty();
        out.inner.insert("UUID".to_owned(), Rc::new(UuidType));
        out.inner.insert("String".to_owned(), Rc::new(StringType));
        out.inner.insert("DateTime".to_owned(), Rc::new(DateTimeType));
        out.inner.insert("Encrypted".to_owned(), Rc::new(EncryptedType));
        out
    }
}

impl TypeStore {
    pub fn empty() -> Self {
        Self {
            inner: HashMap::default()
        }
    }

    pub fn get(&self, name: impl AsRef<str>) -> Option<Rc<dyn Type>> {
        self.inner.get(name.as_ref()).cloned()
    }
}

pub trait Type {}

pub struct UuidType;
impl Type for UuidType {}

pub struct StringType;
impl Type for StringType {}

pub struct DateTimeType;
impl Type for DateTimeType {}

pub struct EncryptedType;
impl Type for EncryptedType {}