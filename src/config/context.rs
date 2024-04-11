use std::rc::Rc;
use hashbrown::HashMap;
use crate::config::{eval};

pub struct Context {
    pub(crate) functions: HashMap<String, Box<dyn eval::Function>>,
}

impl Default for Context {
    fn default() -> Self {
        let mut out = Self::empty();
        eval::build_context(&mut out);
        out
    }
}

impl Context {
    pub fn empty() -> Self {
        Self {
            functions: HashMap::default(),
        }
    }

    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }
}
