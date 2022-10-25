use std::{cell::RefCell, collections::HashMap, rc::Rc};

use futures_signals::signal::{Mutable, Signal};
use gloo_console::log;
use serpent_automation_executor::{
    library::Library,
    run::{CallStack, RunState, ThreadCallStates},
};
use serpent_automation_frontend::ReceiveCallStates;
use silkenweb::{
    node::{element::ElementBuilder, Node},
    prelude::ParentBuilder,
};
use silkenweb_bootstrap::{
    row,
    utility::{Align, Overflow, SetFlex, SetOverflow, SetSpacing, Size::Size3},
};
use thread_view::ThreadView;

mod animation;
mod speech_bubble;
mod thread_view;
mod css {
    silkenweb::css_classes!(visibility: pub, path: "serpent-automation.scss");
}

pub fn app(library: &Rc<Library>, view_call_states: &ViewCallStates) -> impl Into<Node> {
    let main_id = library.main_id().unwrap();

    row()
        .margin(Some(Size3))
        .class(css::FLOW_DIAGRAMS_CONTAINER)
        .align_items(Align::Start)
        .overflow(Overflow::Auto)
        .child(ThreadView::new(main_id, library, view_call_states))
}

#[derive(Clone, Default)]
pub struct ViewCallStates(Rc<RefCell<ViewCallStatesData>>);

impl ViewCallStates {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run_state(&self, call_stack: &CallStack) -> impl Signal<Item = RunState> {
        let mut data = self.0.borrow_mut();

        if let Some(existing) = data.view_call_states.get(call_stack) {
            existing
        } else {
            let new = Mutable::new(data.call_states.run_state(call_stack));
            data.view_call_states
                .entry(call_stack.clone())
                .or_insert(new)
        }
        .signal()
    }
}

impl ReceiveCallStates for ViewCallStates {
    fn set_call_states(&self, thread_state: ThreadCallStates) {
        let mut data = self.0.borrow_mut();

        for (call_stack, run_state) in &data.view_call_states {
            log!(format!("call stack {:?}", call_stack));
            run_state.set_neq(thread_state.run_state(call_stack));
        }

        data.call_states = thread_state;
    }
}

#[derive(Default)]
struct ViewCallStatesData {
    view_call_states: HashMap<CallStack, Mutable<RunState>>,
    call_states: ThreadCallStates,
}
