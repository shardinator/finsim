use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Bank {
    pub id: u64,
    pub name: String,
}

impl Bank {
    pub fn new(id: u64, name: String) -> Self {
        Self { id, name }
    }
}
