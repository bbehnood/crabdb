use std::collections::HashMap;

pub struct Database {
    data: HashMap<String, String>,
}

impl Database {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    pub fn set(&mut self, key: String, value: String) {
        self.data.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.data.get(key).map(String::as_str)
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.data.remove(key).is_some()
    }
}

impl Default for Database {
    fn default() -> Self {
        Self::new()
    }
}
