use std::collections::LinkedList;
use hashbrown::HashMap;

pub struct Scope(LinkedList<ScopeFrame>);

impl Default for Scope {
    fn default() -> Self {
        Self(LinkedList::from([
            ScopeFrame::default(),
        ]))
    }
}

#[derive(Default)]
pub struct ScopeFrame {
    locals: HashMap<String, ()>,
}
