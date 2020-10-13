use crate::state_machine::{CharacterSet, StateMachine};
use reqwest::blocking::Client as HttpClient;
use serenity::{model::channel::Message, prelude::Context};
use std::{collections::HashMap, sync::Arc};

const PREFIX: &'static str = "?";
pub(crate) type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub(crate) type CmdPtr = Arc<dyn for<'m> Fn(Args<'m>) -> Result<()> + Send + Sync>;

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
        let mut state = 0;

        let mut opt_lambda_state: Option<usize> = None;
        let mut opt_final_states = vec![];

        command
            .split(' ')
            .filter(|segment| segment.len() > 0)
            .enumerate()
            .for_each(|(i, segment)| {
                if let Some(name) = key_value_pair(segment) {
                    if let Some(lambda) = opt_lambda_state {
                        state = add_key_value(&name, &mut self.state_machine, lambda);
                        self.state_machine.add_next_state(state, lambda);
                        opt_final_states.push(state);
                    } else {
                        opt_final_states.push(state);
                        state = add_space(&mut self.state_machine, state, i);
                        opt_lambda_state = Some(state);
                        state = add_key_value(&name, &mut self.state_machine, state);
                        self.state_machine
                            .add_next_state(state, opt_lambda_state.unwrap());
                        opt_final_states.push(state);
                    }
                } else {
                    opt_lambda_state = None;
                    opt_final_states.truncate(0);
                    state = add_space(&mut self.state_machine, state, i);

                    if segment.starts_with("```\n") && segment.ends_with("```") {
                        let name = &segment[4..segment.len() - 3];
                        state = add_code_segment_multi_line(name, &mut self.state_machine, state);
                    } else if segment.starts_with("```") && segment.ends_with("```") {
                        let name = &segment[3..segment.len() - 3];
                        state =
                            add_code_segment_single_line(name, &mut self.state_machine, state, 3);
                    } else if segment.starts_with("`") && segment.ends_with("`") {
                        let name = &segment[1..segment.len() - 1];
                        state =
                            add_code_segment_single_line(name, &mut self.state_machine, state, 1);
                    } else if segment.starts_with("{") && segment.ends_with("}") {
                        let name = &segment[1..segment.len() - 1];
                        state = add_dynamic_segment(name, &mut self.state_machine, state);
                    } else if segment.ends_with("...") {
                        let name = &segment[..segment.len() - 3];
                        state = add_remaining_segment(name, &mut self.state_machine, state);
                    } else {
                        segment.chars().for_each(|ch| {
                            state = self.state_machine.add(state, CharacterSet::from_char(ch))
                        });
                    }
                }
            });

        let handler = Arc::new(handler);

        if opt_lambda_state.is_some() {
            opt_final_states.iter().for_each(|state| {
                self.state_machine.set_final_state(*state);
                self.state_machine.set_handler(*state, handler.clone());
            });
        } else {
            self.state_machine.set_final_state(state);
            self.state_machine.set_handler(state, handler.clone());
        }

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

fn key_value_pair(s: &'static str) -> Option<&'static str> {
    s.match_indices("={}")
        .nth(0)
        .map(|pair| {
            let name = &s[0..pair.0];
            if name.len() > 0 {
                Some(name)
            } else {
                None
            }
        })
        .flatten()
}

fn add_space(state_machine: &mut StateMachine, mut state: usize, i: usize) -> usize {
    if i > 0 {
        let mut char_set = CharacterSet::from_char(' ');
        char_set.insert('\n');

        state = state_machine.add(state, char_set);
        state_machine.add_next_state(state, state);
    }
    state
}

fn add_dynamic_segment(
    name: &'static str,
    state_machine: &mut StateMachine,
    mut state: usize,
) -> usize {
    let mut char_set = CharacterSet::any();
    char_set.remove(' ');
    state = state_machine.add(state, char_set);
    state_machine.add_next_state(state, state);
    state_machine.start_parse(state, name);
    state_machine.end_parse(state);

    state
}

fn add_remaining_segment(
    name: &'static str,
    state_machine: &mut StateMachine,
    mut state: usize,
) -> usize {
    let char_set = CharacterSet::any();
    state = state_machine.add(state, char_set);
    state_machine.add_next_state(state, state);
    state_machine.start_parse(state, name);
    state_machine.end_parse(state);

    state
}

fn add_code_segment_multi_line(
    name: &'static str,
    state_machine: &mut StateMachine,
    mut state: usize,
) -> usize {
    state = state_machine.add(state, CharacterSet::from_char('`'));
    state = state_machine.add(state, CharacterSet::from_char('`'));
    state = state_machine.add(state, CharacterSet::from_char('`'));

    let lambda = state;

    let mut char_set = CharacterSet::any();
    char_set.remove('`');
    char_set.remove(' ');
    char_set.remove('\n');
    state = state_machine.add(state, char_set);
    state_machine.add_next_state(state, state);

    state = state_machine.add(state, CharacterSet::from_char('\n'));

    state_machine.add_next_state(lambda, state);

    state = state_machine.add(state, CharacterSet::any());
    state_machine.add_next_state(state, state);
    state_machine.start_parse(state, name);
    state_machine.end_parse(state);

    state = state_machine.add(state, CharacterSet::from_char('`'));
    state = state_machine.add(state, CharacterSet::from_char('`'));
    state = state_machine.add(state, CharacterSet::from_char('`'));

    state
}

fn add_code_segment_single_line(
    name: &'static str,
    state_machine: &mut StateMachine,
    mut state: usize,
    n_backticks: usize,
) -> usize {
    (0..n_backticks).for_each(|_| {
        state = state_machine.add(state, CharacterSet::from_char('`'));
    });
    state = state_machine.add(state, CharacterSet::any());
    state_machine.add_next_state(state, state);
    state_machine.start_parse(state, name);
    state_machine.end_parse(state);
    (0..n_backticks).for_each(|_| {
        state = state_machine.add(state, CharacterSet::from_char('`'));
    });

    state
}

fn add_key_value(name: &'static str, state_machine: &mut StateMachine, mut state: usize) -> usize {
    name.chars().for_each(|c| {
        state = state_machine.add(state, CharacterSet::from_char(c));
    });
    state = state_machine.add(state, CharacterSet::from_char('='));

    let mut char_set = CharacterSet::any();
    char_set.remove(' ');
    char_set.remove('\n');
    state = state_machine.add(state, char_set);
    state_machine.add_next_state(state, state);
    state_machine.start_parse(state, name);
    state_machine.end_parse(state);

    state
}
