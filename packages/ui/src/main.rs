use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use futures::StreamExt;
use futures_signals::signal::{Mutable, Signal, SignalExt};
use gloo_console::log;
use gloo_net::websocket::{futures::WebSocket, Message};
use serpent_automation_executor::{
    library::{FunctionId, Library},
    run::{CallStack, FnStatus, RunTracer},
    syntax_tree::{parse, Expression, Function, Statement},
    CODE,
};
use silkenweb::{
    clone,
    elements::{
        html::{a, div, ABuilder, DivBuilder},
        ElementEvents,
    },
    mount,
    node::element::{Element, ElementBuilder},
    prelude::{HtmlElement, HtmlElementEvents, ParentBuilder},
    task::on_animation_frame,
    value::Sig,
};
use silkenweb_bootstrap::{
    button::{button, ButtonStyle},
    button_group::button_group,
    column,
    dropdown::{dropdown, dropdown_menu, DropdownBuilder},
    icon::{icon, Icon, IconType},
    row,
    utility::{
        Align, Colour, Overflow, SetAlign, SetBorder, SetColour, SetFlex, SetOverflow, SetSpacing,
        Shadow, Side,
        Size::{self, Size3},
    },
};

mod bs {
    silkenweb::css_classes!(visibility: pub, path: "bootstrap.min.css");
}

mod css {
    silkenweb::css_classes!(visibility: pub, path: "serpent-automation.scss");
}

mod icon {
    silkenweb::css_classes!(visibility: pub, path: "bootstrap-icons.css");
}

const BUTTON_STYLE: ButtonStyle = ButtonStyle::Outline(Colour::Secondary);

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

fn render_call(
    name: FunctionId,
    args: &[Expression<FunctionId>],
    is_last: bool,
    library: &Rc<Library>,
    call_stack: &CallStack,
    run_states: &RunStates,
) -> Vec<Element> {
    let mut elems: Vec<Element> = args
        .iter()
        .flat_map(|arg| render_expression(arg, false, library, call_stack, run_states))
        .collect();

    let mut call_stack = call_stack.clone();
    call_stack.push(name);
    elems.push(render_function(
        library.lookup(name),
        is_last,
        library,
        &call_stack,
        run_states,
    ));

    elems
}

fn expression_is_expandable(expression: &Expression<FunctionId>) -> bool {
    match expression {
        Expression::Variable { .. } => false,
        Expression::Call { .. } => true,
    }
}

fn statement_is_expandable(stmt: &Statement<FunctionId>) -> bool {
    match stmt {
        Statement::Pass => false,
        Statement::Expression(e) => expression_is_expandable(e),
    }
}

fn is_expandable(stmts: &[Statement<FunctionId>]) -> bool {
    stmts.iter().any(statement_is_expandable)
}

fn render_function(
    f: &Function<FunctionId>,
    is_last: bool,
    library: &Rc<Library>,
    call_stack: &CallStack,
    run_states: &RunStates,
) -> Element {
    let expanded = is_expandable(f.body()).then(|| Mutable::new(false));
    let header = render_function_header(f.name(), expanded.clone(), call_stack, run_states);
    let header_elem = header.handle().dom_element();
    let mut main = row().align_items(Align::Center).child(header);

    if !is_last {
        main = main.child(horizontal_line()).child(arrow_right());
    }

    if let Some(expanded) = expanded {
        column()
            .align_items(Align::Stretch)
            .child(main)
            .child(render_function_body(
                f.body(),
                header_elem,
                expanded,
                library,
                call_stack,
                run_states,
            ))
            .into()
    } else {
        main.into()
    }
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

fn render_function_body(
    body: &Arc<Vec<Statement<FunctionId>>>,
    parent: web_sys::Element,
    expanded: Mutable<bool>,
    library: &Rc<Library>,
    call_stack: &CallStack,
    run_states: &RunStates,
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
            clone!(body, library, call_stack, run_states);

            move |expanded| {
                if expanded {
                    Some(expanded_body(&body, &library, &call_stack, &run_states))
                } else {
                    style.set(style_min_size(0.0, 0.0));
                    None
                }
            }
        })))
}

fn expanded_body(
    body: &Arc<Vec<Statement<FunctionId>>>,
    library: &Rc<Library>,
    call_stack: &CallStack,
    run_states: &RunStates,
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
        .children(render_body_statements(
            body_head.iter().copied(),
            false,
            library,
            call_stack,
            run_states,
        ))
        .children(render_body_statements(
            body_tail.iter().copied(),
            true,
            library,
            call_stack,
            run_states,
        ));
    row
}

fn render_body_statements<'a>(
    body: impl Iterator<Item = &'a Statement<FunctionId>> + 'a,
    is_last: bool,
    library: &'a Rc<Library>,
    call_stack: &'a CallStack,
    run_states: &'a RunStates,
) -> impl Iterator<Item = Element> + 'a {
    body.flat_map(move |statement| match statement {
        Statement::Pass => Vec::new(),
        Statement::Expression(expr) => {
            render_expression(expr, is_last, library, call_stack, run_states)
        }
    })
}

fn render_function_header(
    name: &str,
    expanded: Option<Mutable<bool>>,
    call_stack: &CallStack,
    run_states: &RunStates,
) -> Element {
    let status_signal = run_states
        .borrow_mut()
        .entry(call_stack.clone())
        .or_insert_with(|| Mutable::new(FnStatus::NotRun))
        .signal();

    if let Some(expanded) = expanded {
        button_group(format!("Function {name}"))
            .shadow(Shadow::Medium)
            .dropdown(fn_dropdown(name, status_signal))
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
        fn_dropdown(name, status_signal)
            .shadow(Shadow::Medium)
            .into()
    }
}

fn render_expression(
    expr: &Expression<FunctionId>,
    is_last: bool,
    library: &Rc<Library>,
    call_stack: &CallStack,
    run_states: &RunStates,
) -> Vec<Element> {
    match expr {
        Expression::Variable { .. } => Vec::new(),
        Expression::Call { name, args } => {
            render_call(*name, args, is_last, library, call_stack, run_states)
        }
    }
}

async fn server_connection(mut server_ws: WebSocket, run_states: RunStates) {
    log!("Connected to websocket");

    while let Some(msg) = server_ws.next().await {
        log!(format!("Received: {:?}", msg));

        match msg.unwrap() {
            Message::Text(text) => {
                let run_tracer: RunTracer = serde_json_wasm::from_str(&text).unwrap();
                log!(format!("Deserialized `RunTracer` from `{text}`"));

                // TODO: Share current `run_tracer` so we can get status of newly expanded
                // function bodies.
                for (call_stack, status) in run_states.borrow().iter() {
                    log!(format!("call stack {:?}", call_stack));
                    status.set_neq(run_tracer.status(call_stack));
                }
            }
            Message::Bytes(_) => log!("Unknown binary message"),
        }
    }

    log!("WebSocket Closed")
}

// TODO: Struct for this
type RunStates = Rc<RefCell<HashMap<CallStack, Mutable<FnStatus>>>>;

fn main() {
    let module = parse(CODE).unwrap();
    let library = Rc::new(Library::link(module));
    let run_states: RunStates = Rc::new(RefCell::new(HashMap::new()));
    let server = WebSocket::open("ws://127.0.0.1:9090/").unwrap();

    let app = row()
        .margin(Some(Size3))
        .class(css::FLOW_DIAGRAMS_CONTAINER)
        .align_items(Align::Start)
        .overflow(Overflow::Auto)
        .children([render_function(
            library.main().unwrap(),
            true,
            &library,
            &vec![library.main_id().unwrap()],
            &run_states,
        )])
        .spawn_future(server_connection(server, run_states.clone()));

    mount("app", app);
}
