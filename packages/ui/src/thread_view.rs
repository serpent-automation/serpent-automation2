use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use derive_more::Into;
use futures_signals::signal::{Mutable, Signal, SignalExt};
use serpent_automation_executor::{
    library::{FunctionId, Library},
    run::{CallStack, FnStatus},
    syntax_tree::{Expression, Function, Statement},
};
use serpent_automation_frontend::{is_expandable, statement_is_expandable, StackFrameStates};
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
    prelude::{HtmlElement, HtmlElementEvents, ParentBuilder},
    task::on_animation_frame,
    value::Sig,
    Value,
};
use silkenweb_bootstrap::{
    button::{button, ButtonStyle},
    button_group::button_group,
    column,
    dropdown::{dropdown, dropdown_menu, DropdownBuilder},
    icon::{icon, Icon, IconType},
    row,
    utility::{
        Align, Colour, SetAlign, SetBorder, SetColour, SetFlex, SetSpacing, Shadow, Side,
        Size::{self, Size3},
    },
};

use crate::css;

const BUTTON_STYLE: ButtonStyle = ButtonStyle::Outline(Colour::Secondary);

#[derive(Into, Value)]
pub struct ThreadView(Node);

impl ThreadView {
    // TODO: Return Result<Self, LinkError>
    pub fn new(
        fn_id: FunctionId,
        library: &Rc<Library>,
        stack_frame_states: &StackFrameStates,
    ) -> Self {
        let fn_nodes = FnNodes::new(stack_frame_states.clone(), library.clone());
        Self(function(fn_id, true, vec![], &fn_nodes).into())
    }
}

#[derive(Clone)]
struct FnNodes {
    expanded: Rc<RefCell<HashMap<CallStack, Mutable<bool>>>>,
    stack_frame_states: StackFrameStates,
    library: Rc<Library>,
}

impl FnNodes {
    fn new(stack_frame_states: StackFrameStates, library: Rc<Library>) -> Self {
        Self {
            expanded: Rc::new(RefCell::new(HashMap::new())),
            library,
            stack_frame_states,
        }
    }

    fn expanded(&self, call_stack: &CallStack) -> Mutable<bool> {
        self.expanded
            .borrow_mut()
            .entry(call_stack.clone())
            .or_insert_with(|| Mutable::new(false))
            .clone()
    }

    fn status(&self, call_stack: &CallStack) -> impl Signal<Item = FnStatus> {
        self.stack_frame_states.status(call_stack)
    }

    fn lookup_fn(&self, fn_id: FunctionId) -> &Function<FunctionId> {
        self.library.lookup(fn_id)
    }
}

fn function(
    fn_id: FunctionId,
    is_last: bool,
    mut call_stack: CallStack,
    fn_nodes: &FnNodes,
) -> Element {
    let f = fn_nodes.lookup_fn(fn_id);
    call_stack.push(fn_id);
    let expanded = is_expandable(f.body()).then(|| fn_nodes.expanded(&call_stack));
    let status = fn_nodes.status(&call_stack);
    let header = function_header(f.name(), expanded.clone(), status);
    let header_elem = header.handle().dom_element();
    let mut main = row().align_items(Align::Center).child(header);

    if !is_last {
        main = main.child(horizontal_line()).child(arrow_right());
    }

    if let Some(expanded) = expanded {
        column()
            .align_items(Align::Stretch)
            .child(main)
            .child(function_body(
                f.body(),
                header_elem,
                expanded,
                call_stack,
                fn_nodes,
            ))
            .into()
    } else {
        main.into()
    }
}

fn fn_dropdown(name: &str, fn_status: impl Signal<Item = FnStatus> + 'static) -> DropdownBuilder {
    let status = fn_status.map(|status| {
        match status {
            FnStatus::NotRun => Icon::circle().colour(Colour::Secondary),
            FnStatus::Running => Icon::play_circle_fill().colour(Colour::Primary),
            FnStatus::Ok => Icon::check_circle_fill().colour(Colour::Success),
            FnStatus::Error => Icon::exclamation_triangle_fill().colour(Colour::Danger),
        }
        .margin_on_side((Some(Size::Size2), Side::End))
    });

    dropdown(
        button("button", ButtonStyle::Outline(Colour::Secondary))
            .icon(Sig(status))
            .text(name),
        dropdown_menu().children([dropdown_item("Run"), dropdown_item("Pause")]),
    )
}

fn dropdown_item(name: &str) -> ABuilder {
    a().href("#").text(name)
}

fn horizontal_line() -> Element {
    div().class(css::HORIZONTAL_LINE).into()
}

fn arrow_right() -> Element {
    div()
        .class(css::ARROW_HEAD_RIGHT)
        .background_colour(Colour::Secondary)
        .into()
}

fn call(
    name: FunctionId,
    args: &[Expression<FunctionId>],
    is_last: bool,
    call_stack: CallStack,
    fn_nodes: &FnNodes,
) -> Vec<Element> {
    let mut elems: Vec<Element> = args
        .iter()
        .flat_map(|arg| expression(arg, false, &call_stack, fn_nodes))
        .collect();

    elems.push(function(name, is_last, call_stack, fn_nodes));

    elems
}

fn style_size(bound: &str, width: f64, height: f64) -> String {
    format!("overflow: hidden; {bound}-width: {width}px; {bound}-height: {height}px",)
}

fn style_max_size(width: f64, height: f64) -> String {
    style_size("max", width, height)
}

fn style_min_size(width: f64, height: f64) -> String {
    style_size("min", width, height)
}

fn function_body(
    body: &Arc<Vec<Statement<FunctionId>>>,
    parent: web_sys::Element,
    expanded: Mutable<bool>,
    call_stack: CallStack,
    fn_nodes: &FnNodes,
) -> DivBuilder {
    let style = Mutable::new("".to_owned());
    let show_body = Mutable::new(false);

    div()
        .align_self(Align::Start)
        .class(css::TRANSITION_ALL)
        .spawn_future(expanded.signal().for_each({
            clone!(show_body);
            move |expanded| {
                if expanded {
                    show_body.set(true);
                }
                async {}
            }
        }))
        .effect_signal(expanded.signal(), {
            clone!(style, show_body);
            move |elem, expanded| {
                let elem_bounds = elem.get_bounding_client_rect();

                if expanded {
                    let initial_width = parent.get_bounding_client_rect().width();
                    let final_bounds = elem.get_bounding_client_rect();
                    style.set(style_max_size(initial_width, 0.0));

                    on_animation_frame({
                        clone!(style);
                        move || {
                            style.set(style_max_size(final_bounds.width(), final_bounds.height()));
                        }
                    })
                } else {
                    style.set(style_min_size(elem_bounds.width(), elem_bounds.height()));

                    on_animation_frame({
                        clone!(show_body);
                        move || show_body.set(false)
                    });
                }
            }
        })
        .on_transitionend({
            clone!(style);
            move |_, _| style.set("".to_owned())
        })
        .style(Sig(style.signal_cloned()))
        .optional_child(Sig(show_body.signal().map({
            clone!(body, fn_nodes);

            move |expanded| {
                if expanded {
                    Some(expanded_body(&body, &call_stack, &fn_nodes))
                } else {
                    style.set(style_min_size(0.0, 0.0));
                    None
                }
            }
        })))
}

fn expanded_body(
    body: &Arc<Vec<Statement<FunctionId>>>,
    call_stack: &CallStack,
    fn_nodes: &FnNodes,
) -> DivBuilder {
    let body: Vec<_> = body
        .iter()
        .filter(|stmt| statement_is_expandable(stmt))
        .collect();
    assert!(!body.is_empty());
    let (body_head, body_tail) = body.split_at(body.len() - 1);
    assert!(body_tail.len() == 1);

    let row = row()
        .align_items(Align::Start)
        .class(css::SPEECH_BUBBLE_BELOW)
        .margin_on_side((Some(Size3), Side::Top))
        .margin_on_side((Some(Size3), Side::End))
        .padding(Size3)
        .border(true)
        .border_colour(Colour::Secondary)
        .rounded_border(true)
        .shadow(Shadow::Medium)
        .children(body_statements(
            body_head.iter().copied(),
            false,
            call_stack,
            fn_nodes,
        ))
        .children(body_statements(
            body_tail.iter().copied(),
            true,
            call_stack,
            fn_nodes,
        ));
    row
}

fn body_statements<'a>(
    body: impl Iterator<Item = &'a Statement<FunctionId>> + 'a,
    is_last: bool,
    call_stack: &'a CallStack,
    fn_nodes: &'a FnNodes,
) -> impl Iterator<Item = Element> + 'a {
    body.flat_map(move |statement| match statement {
        Statement::Pass => Vec::new(),
        Statement::Expression(expr) => expression(expr, is_last, call_stack, fn_nodes),
    })
}

fn function_header(
    name: &str,
    expanded: Option<Mutable<bool>>,
    status: impl Signal<Item = FnStatus> + 'static,
) -> Element {
    if let Some(expanded) = expanded {
        button_group(format!("Function {name}"))
            .shadow(Shadow::Medium)
            .dropdown(fn_dropdown(name, status))
            .button(
                button("button", BUTTON_STYLE)
                    .on_click({
                        clone!(expanded);
                        move |_, _| {
                            expanded.replace_with(|e| !*e);
                        }
                    })
                    .icon(icon(Sig(expanded.signal().map(|expanded| {
                        if expanded {
                            IconType::ZoomOut
                        } else {
                            IconType::ZoomIn
                        }
                    })))),
            )
            .into()
    } else {
        fn_dropdown(name, status).shadow(Shadow::Medium).into()
    }
}

fn expression(
    expr: &Expression<FunctionId>,
    is_last: bool,
    call_stack: &CallStack,
    fn_nodes: &FnNodes,
) -> Vec<Element> {
    match expr {
        Expression::Variable { .. } => Vec::new(),
        Expression::Call { name, args } => call(*name, args, is_last, call_stack.clone(), fn_nodes),
    }
}
