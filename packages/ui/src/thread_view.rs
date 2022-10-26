use std::{cell::RefCell, collections::HashMap, iter, rc::Rc};

use derive_more::Into;
use futures_signals::signal::{Mutable, Signal, SignalExt};
use serpent_automation_executor::{
    library::{FunctionId, Library},
    run::{CallStack, RunState, StackFrame},
    syntax_tree::{Expression, LinkedBody, LinkedFunction, Statement},
};
use serpent_automation_frontend::{is_expandable, statement_is_expandable};
use silkenweb::{
    clone,
    elements::{
        html::{a, div, ABuilder, DivBuilder},
        ElementEvents,
    },
    node::{
        element::{Element, ElementBuilder},
        Node,
    },
    prelude::ParentBuilder,
    value::Sig,
    Value,
};
use silkenweb_bootstrap::{
    button::{icon_button, ButtonStyle},
    button_group::button_group,
    column,
    dropdown::{dropdown, dropdown_menu, DropdownBuilder},
    icon::{icon, Icon, IconType},
    row,
    utility::{
        Align, Colour, SetBorder, SetColour, SetFlex, SetSpacing, Shadow, Side,
        Size::{self},
    },
};

use crate::{
    animation::AnimatedExpand, css, speech_bubble::SpeechBubble, thread_view::conditional::if_node,
    ViewCallStates,
};

mod conditional;

#[derive(Into, Value)]
pub struct ThreadView(Node);

impl ThreadView {
    // TODO: Return Result<Self, LinkError>
    pub fn new(
        fn_id: FunctionId,
        library: &Rc<Library>,
        view_call_states: &ViewCallStates,
    ) -> Self {
        let view_state = ThreadViewState::new(view_call_states.clone(), library.clone());
        Self(function_node(fn_id, true, CallStack::new(), &view_state).into())
    }
}

#[derive(Clone)]
struct ThreadViewState {
    expanded: Rc<RefCell<HashMap<CallStack, Mutable<bool>>>>,
    view_call_states: ViewCallStates,
    library: Rc<Library>,
}

impl ThreadViewState {
    fn new(view_call_states: ViewCallStates, library: Rc<Library>) -> Self {
        Self {
            expanded: Rc::new(RefCell::new(HashMap::new())),
            library,
            view_call_states,
        }
    }

    fn expanded(&self, call_stack: &CallStack) -> Mutable<bool> {
        self.expanded
            .borrow_mut()
            .entry(call_stack.clone())
            .or_insert_with(|| Mutable::new(false))
            .clone()
    }

    fn run_state(&self, call_stack: &CallStack) -> impl Signal<Item = RunState> {
        self.view_call_states.run_state(call_stack)
    }

    fn lookup_fn(&self, fn_id: FunctionId) -> &LinkedFunction {
        self.library.lookup(fn_id)
    }
}

fn function_node(
    fn_id: FunctionId,
    is_last: bool,
    mut call_stack: CallStack,
    view_state: &ThreadViewState,
) -> Element {
    let f = view_state.lookup_fn(fn_id);
    call_stack.push(StackFrame::Function(fn_id));
    let name = f.name();
    let body = match f.body() {
        LinkedBody::Local(body) => is_expandable(body).then_some(body),
        LinkedBody::Python => None,
    };
    let run_state = view_state.run_state(&call_stack);

    if let Some(body) = body {
        let expanded = view_state.expanded(&call_stack);
        clone!(body, call_stack, view_state);
        let body = move || {
            row().align_items(Align::Stretch).children(body_statements(
                body.iter(),
                &call_stack,
                &view_state,
            ))
        };

        expandable_node(name, FUNCTION_STYLE, run_state, is_last, expanded, body)
    } else {
        leaf_node(name, FUNCTION_STYLE, run_state, is_last)
    }
}

fn expandable_node<Elem>(
    type_name: &str,
    style: ButtonStyle,
    run_state: impl Signal<Item = RunState> + 'static,
    is_last: bool,
    is_expanded: Mutable<bool>,
    mut expanded: impl FnMut() -> Elem + 'static,
) -> Element
where
    Elem: Into<Element>,
{
    node_column(
        button_group(type_name)
            .shadow(Shadow::Medium)
            .dropdown(item_dropdown(type_name, style, run_state))
            .button(zoom_button(&is_expanded, style)),
        is_last,
    )
    .child(
        row()
            .align_items(Align::Start)
            .class(css::EXPANDABLE_NODE)
            .animated_expand(
                move || div().speech_bubble().child(expanded().into()),
                is_expanded,
            ),
    )
    .into()
}

fn leaf_node(
    name: &str,
    style: ButtonStyle,
    run_state: impl Signal<Item = RunState> + 'static,
    is_last: bool,
) -> Element {
    node_column(
        item_dropdown(name, style, run_state).shadow(Shadow::Medium),
        is_last,
    )
    .into()
}

fn node_column(header: impl Into<Element>, is_last: bool) -> DivBuilder {
    let main = row().align_items(Align::Center).child(header.into());

    column().align_items(Align::Stretch).child(if is_last {
        main
    } else {
        let horizontal_line = div().class(css::HORIZONTAL_LINE);
        let arrow_head = div()
            .class(css::ARROW_HEAD_RIGHT)
            .background_colour(Colour::Secondary);
        main.child(horizontal_line).child(arrow_head)
    })
}

fn item_dropdown(
    name: &str,
    style: ButtonStyle,
    run_state: impl Signal<Item = RunState> + 'static,
) -> DropdownBuilder {
    let run_state = run_state.map(|run_state| {
        match run_state {
            RunState::NotRun => Icon::circle().colour(Colour::Secondary),
            RunState::Running => Icon::play_circle_fill().colour(Colour::Primary),
            RunState::Successful | RunState::PredicateSuccessful(true) => {
                Icon::check_circle_fill().colour(Colour::Success)
            }
            RunState::PredicateSuccessful(false) => Icon::circle_fill().colour(Colour::Success),
            RunState::Failed => Icon::exclamation_triangle_fill().colour(Colour::Danger),
        }
        .margin_on_side((Some(Size::Size2), Side::End))
    });

    dropdown(
        icon_button("button", Sig(run_state), style).text(name),
        dropdown_menu().children([dropdown_item("Run"), dropdown_item("Pause")]),
    )
}

fn dropdown_item(name: &str) -> ABuilder {
    a().href("#").text(name)
}

fn zoom_button(
    expanded: &Mutable<bool>,
    style: ButtonStyle,
) -> silkenweb_bootstrap::button::ButtonBuilder {
    icon_button(
        "button",
        icon(Sig(expanded.signal().map(|expanded| {
            if expanded {
                IconType::ZoomOut
            } else {
                IconType::ZoomIn
            }
        }))),
        style,
    )
    .on_click({
        clone!(expanded);
        move |_, _| {
            expanded.replace_with(|e| !*e);
        }
    })
}

fn call<'a>(
    name: FunctionId,
    args: &'a [Expression<FunctionId>],
    is_last: bool,
    call_stack: CallStack,
    view_state: &'a ThreadViewState,
) -> impl Iterator<Item = Element> + 'a {
    args.iter()
        .enumerate()
        .flat_map({
            clone!(mut call_stack);

            move |(arg_index, arg)| {
                // TODO: Push and pop call stack for efficiency
                call_stack.push(StackFrame::Argument(arg_index));
                expression(arg, false, &call_stack, view_state)
            }
        })
        .chain(iter::once(function_node(
            name, is_last, call_stack, view_state,
        )))
}

fn expression(
    expr: &Expression<FunctionId>,
    is_last: bool,
    call_stack: &CallStack,
    view_state: &ThreadViewState,
) -> Vec<Element> {
    match expr {
        Expression::Variable { .. } | Expression::Literal(_) => Vec::new(),
        Expression::Call { name, args } => {
            call(*name, args, is_last, call_stack.clone(), view_state).collect()
        }
    }
}

fn body_statements<'a>(
    body: impl Iterator<Item = &'a Statement<FunctionId>>,
    call_stack: &'a CallStack,
    view_state: &'a ThreadViewState,
) -> impl Iterator<Item = Element> + 'a {
    let body: Vec<_> = body
        .enumerate()
        .filter(|(_index, stmt)| statement_is_expandable(stmt))
        .collect();
    assert!(!body.is_empty());
    let last_index = body.len() - 1;

    body.into_iter()
        .enumerate()
        .flat_map(move |(index, (stmt_index, statement))| {
            let is_last = index == last_index;
            clone!(mut call_stack);
            call_stack.push(StackFrame::Statement(stmt_index));

            match statement {
                Statement::Pass => Vec::new(),
                Statement::Expression(expr) => expression(expr, is_last, &call_stack, view_state),
                Statement::If {
                    condition,
                    then_block,
                    else_block,
                } => vec![if_node(
                    condition.clone(),
                    then_block.clone(),
                    else_block.clone(),
                    is_last,
                    &call_stack,
                    view_state,
                )],
            }
            .into_iter()
        })
}

const FUNCTION_STYLE: ButtonStyle = ButtonStyle::Outline(Colour::Secondary);
