use crate::state_machine::{CharacterSet, StateMachine};
use reqwest::blocking::Client as HttpClient;
use serenity::{model::channel::Message, prelude::Context};
use std::collections::HashMap;

const PREFIX: &'static str = "?";
pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub(crate) type CmdPtr = Box<dyn for<'m> Fn(Args<'m>) -> Result<()> + Send + Sync>;

pub struct Args<'m> {
    pub http: &'m HttpClient,
    pub cx: &'m Context,
    pub msg: &'m Message,
    pub params: HashMap<&'m str, &'m str>,
}

pub(crate) struct Commands {
    state_machine: StateMachine,
    client: HttpClient,
    menu: Option<String>,
}

impl Commands {
    pub(crate) fn new() -> Self {
        Self {
            state_machine: StateMachine::new(),
            client: HttpClient::new(),
            menu: Some(String::new()),
        }
    }

    pub(crate) fn add(
        &mut self,
        command: &'static str,
        handler: impl Fn(Args) -> Result<()> + Send + Sync + 'static,
    ) {
        info!("Adding command {}", &command);
        let mut param_names = Vec::new();
        let mut state = 0;

        command
            .split(' ')
            .filter(|segment| segment.len() > 0)
            .enumerate()
            .for_each(|(i, segment)| {
                if segment.starts_with("[") && segment.ends_with("]") {
                    state = add_space(&mut self.state_machine, state, i);
                    state = add_quoted_dynamic_segment(&mut self.state_machine, state);
                    param_names.push(&segment[1..segment.len() - 1]);
                } else if segment.starts_with("{") && segment.ends_with("}") {
                    state = add_space(&mut self.state_machine, state, i);
                    state = add_dynamic_segment(&mut self.state_machine, state);
                    param_names.push(&segment[1..segment.len() - 1]);
                } else if segment.ends_with("...") {
                    state = add_space(&mut self.state_machine, state, i);
                    state = add_remaining_segment(&mut self.state_machine, state);
                    param_names.push(&segment[..segment.len() - 3]);
                } else {
                    state = add_space(&mut self.state_machine, state, i);
                    segment.chars().for_each(|ch| {
                        state = self.state_machine.add(state, CharacterSet::from_char(ch))
                    });
                }
            });

        self.state_machine.set_final_state(state);
        self.state_machine.set_handler(state, Box::new(handler));
        self.state_machine.set_param_names(state, param_names);
        self.menu.as_mut().map(|menu| {
            *menu += command;
            *menu += "\n"
        });
    }

    pub(crate) fn menu(&mut self) -> Option<String> {
        self.menu.as_mut().map(|menu| *menu += "?help\n");
        self.menu.take()
    }

    pub(crate) fn execute<'m>(&'m self, cx: Context, msg: Message) {
        let message = &msg.content;
        if !msg.is_own(&cx) && message.starts_with(PREFIX) {
            self.state_machine.process(message).map(|matched| {
                info!("Executing command {}", message);
                let args = Args {
                    http: &self.client,
                    cx: &cx,
                    msg: &msg,
                    params: matched.params,
                };
                if let Err(e) = (matched.handler)(args) {
                    println!("{}", e);
                }
            });
        }
    }
}

#[inline]
fn add_space(state_machine: &mut StateMachine, mut state: usize, i: usize) -> usize {
    if i > 0 {
        state = state_machine.add(state, CharacterSet::from_char(' '));
        state_machine.add_next_state(state, state);
    }
    state
}

#[inline]
fn add_dynamic_segment(state_machine: &mut StateMachine, mut state: usize) -> usize {
    let mut char_set = CharacterSet::any();
    char_set.remove(' ');
    state = state_machine.add(state, char_set);
    state_machine.add_next_state(state, state);
    state_machine.start_parse(state);
    state_machine.end_parse(state);

    state
}

#[inline]
fn add_remaining_segment(state_machine: &mut StateMachine, mut state: usize) -> usize {
    let char_set = CharacterSet::any();
    state = state_machine.add(state, char_set);
    state_machine.add_next_state(state, state);
    state_machine.start_parse(state);
    state_machine.end_parse(state);

    state
}

#[inline]
fn add_quoted_dynamic_segment(state_machine: &mut StateMachine, mut state: usize) -> usize {
    state = state_machine.add(state, CharacterSet::from_char('"'));
    state = state_machine.add(state, CharacterSet::any());
    state_machine.add_next_state(state, state);
    state_machine.start_parse(state);
    state_machine.end_parse(state);
    state = state_machine.add(state, CharacterSet::from_char('"'));

    state
}
