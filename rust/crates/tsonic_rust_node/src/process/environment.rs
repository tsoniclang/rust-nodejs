use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::error::{NodeError, NodeResult};

#[derive(Debug, Clone)]
enum EnvironmentStorage {
    Process,
    Owned(Rc<RefCell<BTreeMap<String, Option<String>>>>),
}

#[derive(Debug, Clone)]
pub struct ProcessEnv {
    storage: EnvironmentStorage,
}

impl Default for ProcessEnv {
    fn default() -> Self {
        Self {
            storage: EnvironmentStorage::Owned(Rc::new(RefCell::new(BTreeMap::new()))),
        }
    }
}

impl ProcessEnv {
    pub fn get(&self, name: &str) -> Option<String> {
        match &self.storage {
            EnvironmentStorage::Process => super::env_get(name),
            EnvironmentStorage::Owned(values) => values.borrow().get(name).cloned().flatten(),
        }
    }

    pub fn set(&self, name: &str, value: Option<String>) -> NodeResult<()> {
        match &self.storage {
            EnvironmentStorage::Process => {
                let value = value.unwrap_or_else(|| "undefined".to_owned());
                if name.is_empty() || name.contains(['=', '\0']) || value.contains('\0') {
                    return Err(NodeError::new(
                        "ERR_INVALID_ARG_VALUE",
                        "invalid process environment entry",
                    ));
                }
                super::env_set(name, &value);
            }
            EnvironmentStorage::Owned(values) => {
                values.borrow_mut().insert(name.to_owned(), value);
            }
        }
        Ok(())
    }

    pub fn entries(&self) -> BTreeMap<String, String> {
        match &self.storage {
            EnvironmentStorage::Process => std::env::vars_os()
                .map(|(name, value)| {
                    (
                        name.to_string_lossy().into_owned(),
                        value.to_string_lossy().into_owned(),
                    )
                })
                .collect(),
            EnvironmentStorage::Owned(values) => values
                .borrow()
                .iter()
                .filter_map(|(name, value)| {
                    value.as_ref().map(|value| (name.clone(), value.clone()))
                })
                .collect(),
        }
    }
}

pub fn environment() -> ProcessEnv {
    ProcessEnv {
        storage: EnvironmentStorage::Process,
    }
}
