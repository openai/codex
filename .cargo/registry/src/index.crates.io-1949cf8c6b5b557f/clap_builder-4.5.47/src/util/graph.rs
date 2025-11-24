#[derive(Debug)]
struct Child<T> {
    id: T,
    children: Vec<usize>,
}

impl<T> Child<T> {
    fn new(id: T) -> Self {
        Child {
            id,
            children: vec![],
        }
    }
}

#[derive(Debug)]
pub(crate) struct ChildGraph<T>(Vec<Child<T>>);

impl<T> ChildGraph<T>
where
    T: Sized + PartialEq + Clone,
{
    pub(crate) fn with_capacity(s: usize) -> Self {
        ChildGraph(Vec::with_capacity(s))
    }

    pub(crate) fn insert(&mut self, req: T) -> usize {
        self.0.iter().position(|e| e.id == req).unwrap_or_else(|| {
            let idx = self.0.len();
            self.0.push(Child::new(req));
            idx
        })
    }

    pub(crate) fn insert_child(&mut self, parent: usize, child: T) -> usize {
        let c_idx = self.0.len();
        self.0.push(Child::new(child));
        self.0[parent].children.push(c_idx);
        c_idx
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter().map(|r| &r.id)
    }

    pub(crate) fn contains(&self, req: &T) -> bool {
        self.0.iter().any(|r| r.id == *req)
    }
}
