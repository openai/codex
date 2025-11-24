pub(crate) struct Pretty {
    in_value: bool,
}

impl Pretty {
    pub(crate) fn new() -> Self {
        Self { in_value: false }
    }
}

impl crate::visit_mut::VisitMut for Pretty {
    fn visit_document_mut(&mut self, node: &mut crate::DocumentMut) {
        crate::visit_mut::visit_document_mut(self, node);
    }

    fn visit_item_mut(&mut self, node: &mut crate::Item) {
        if !self.in_value {
            node.make_item();
        }

        crate::visit_mut::visit_item_mut(self, node);
    }

    fn visit_table_mut(&mut self, node: &mut crate::Table) {
        node.decor_mut().clear();

        // Empty tables could be semantically meaningful, so make sure they are not implicit
        if !node.is_empty() {
            node.set_implicit(true);
        }

        crate::visit_mut::visit_table_mut(self, node);
    }

    fn visit_value_mut(&mut self, node: &mut crate::Value) {
        node.decor_mut().clear();

        let old_in_value = self.in_value;
        self.in_value = true;
        crate::visit_mut::visit_value_mut(self, node);
        self.in_value = old_in_value;
    }

    fn visit_array_mut(&mut self, node: &mut crate::Array) {
        crate::visit_mut::visit_array_mut(self, node);

        if (0..=1).contains(&node.len()) {
            node.set_trailing("");
            node.set_trailing_comma(false);
        } else {
            for item in node.iter_mut() {
                item.decor_mut().set_prefix("\n    ");
            }
            node.set_trailing("\n");
            node.set_trailing_comma(true);
        }
    }
}
