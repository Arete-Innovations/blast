use crate::state::{ResourceState, Verb};

#[derive(Debug, Clone, Copy)]
pub struct ResourcePlan {
    pub emit_form: bool,
    pub emit_list: bool,
}

impl ResourcePlan {
    pub fn from(r: &ResourceState) -> Self {
        let has_create = r.verbs.contains_key(&Verb::Create);
        let has_update = r.verbs.contains_key(&Verb::Update);
        let has_list = r.verbs.contains_key(&Verb::List);
        Self {
            emit_form: has_create || has_update,
            emit_list: has_list,
        }
    }

    pub fn has_any(&self) -> bool {
        self.emit_form || self.emit_list
    }
}
