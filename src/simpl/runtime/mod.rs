use std::collections::LinkedList;
use hashbrown::HashMap;
use crate::simpl::parser::SimplFile;

pub struct Globals {

}

pub struct Runtime {
    scope: Scope,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            scope: Scope::default(),
        }
    }
}

struct Scope(LinkedList<ScopeFrame>);

impl Default for Scope {
    fn default() -> Self {
        Self(LinkedList::from([
            ScopeFrame::default(),
        ]))
    }
}

#[derive(Default)]
struct ScopeFrame {
    locals: HashMap<String, ()>,
}

impl Runtime {
    pub fn run(&mut self, _file: &SimplFile) -> Result<(), ()> {
        todo!()
    }
}