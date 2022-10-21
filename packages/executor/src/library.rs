use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use crate::{
    run::ThreadState,
    syntax_tree::{Expression, Function, IdMap, Module},
};

pub struct Library {
    main_id: Option<FunctionId>,
    lookup_map: Vec<Function<FunctionId>>,
}

impl Library {
    /// Constructor
    ///
    /// Translate all `String` function id's to a [`FunctionId`] that is fast to
    /// lookup
    pub fn link(module: Module) -> Self {
        let mut id_map = IdMap::new();
        let mut main_id = None;

        for function in module.functions() {
            let name = function.name();
            let id = FunctionId(id_map.len());
            id_map.insert(name.to_owned(), id);

            if name == "main" {
                main_id = Some(id);
            }
        }

        let lookup_map = module
            .functions()
            .iter()
            .map(|f| f.translate_ids(&id_map))
            .collect();

        Self {
            main_id,
            lookup_map,
        }
    }

    /// Lookup a function id
    ///
    /// Any [`FunctionId`]'s in the return value will be valid lookup with this
    /// function.
    ///
    /// # Panic
    ///
    /// If `id` was not found.
    pub fn lookup(&self, id: FunctionId) -> &Function<FunctionId> {
        // TODO: Return LinkError if not found
        &self.lookup_map[id.0]
    }

    /// Lookup a function called "main"
    ///
    /// Returns `None` if not found.
    pub fn main(&self) -> Option<&Function<FunctionId>> {
        self.main_id.map(|main| self.lookup(main))
    }

    /// The id of a function called "main"
    ///
    /// Returns `None` if there was no "main" function.
    pub fn main_id(&self) -> Option<FunctionId> {
        self.main_id
    }

    pub fn run(&self, thread_state: &watch::Sender<ThreadState>) {
        if let Some(main_id) = self.main_id() {
            Expression::Call {
                name: main_id,
                args: Vec::new(),
            }
            .run(self, thread_state);
        }
    }
}

/// An id for a function that is fast to lookup.
#[derive(Eq, PartialEq, Hash, Copy, Clone, Serialize, Deserialize, Debug)]
pub struct FunctionId(usize);
