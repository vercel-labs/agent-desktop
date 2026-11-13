use std::{cell::Cell, rc::Rc};

#[derive(Debug)]
pub(super) struct Lineage<T> {
    pub(super) element: T,
    pub(super) path: String,
    pub(super) parent: Option<Rc<Lineage<T>>>,
    pub(super) container_verdict: Cell<Option<bool>>,
}

impl<T> Lineage<T> {
    pub(super) fn root(element: T, path: String) -> Rc<Self> {
        Rc::new(Self {
            element,
            path,
            parent: None,
            container_verdict: Cell::new(None),
        })
    }

    pub(super) fn child(parent: &Rc<Self>, element: T, index: usize) -> Rc<Self> {
        Rc::new(Self {
            element,
            path: format!("{}/children/{index}", parent.path),
            parent: Some(Rc::clone(parent)),
            container_verdict: Cell::new(None),
        })
    }
}
