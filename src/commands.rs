use crate::state_machine::{CharacterSet, StateMachine};
use serenity::{model::channel::Message, prelude::Context};
use std::collections::HashMap;

pub(crate) type Result = std::result::Result<(), Box<std::error::Error>>;
pub(crate) type CmdPtr = for<'m> fn(Args<'m>) -> Result;

pub struct Args<'m> {
    pub cx: Context,
    pub msg: Message,
    pub params: HashMap<&'m str, &'m str>,
}

pub(crate) struct Commands {
    state_machine: StateMachine,
}

impl Commands {
    pub(crate) fn new() -> Self {
        Self {
            state_machine: StateMachine::new(),
        }
    }

    pub(crate) fn add(&mut self, command: &'static str, handler: CmdPtr) {
        let mut param_names = Vec::new();
        let mut state = 0;

        command
            .split(' ')
            .filter(|segment| segment.len() > 0)
            .for_each(|segment| {
                if segment.starts_with("[") && segment.ends_with("]") {
                    state = add_multi_part_dynamic_segment(&mut self.state_machine, state);
                    param_names.push(&segment[1..segment.len() - 1]);
                } else if segment.starts_with("{") && segment.ends_with("}") {
                    state = add_dynamic_segment(&mut self.state_machine, state);
                    param_names.push(&segment[1..segment.len() - 1]);
                } else {
                    segment.chars().for_each(|ch| {
                        state = self.state_machine.add(state, CharacterSet::from_char(ch))
                    });
                }
            });

        self.state_machine.set_final_state(state);
        self.state_machine.set_handler(state, handler);
        self.state_machine.set_param_names(state, param_names);
    }

    pub(crate) fn execute<'m>(&'m self, cx: Context, msg: Message) {
        if let Some(cmd) = self.state_machine.process(&msg.content.clone()) {
            let args = Args {
                cx,
                msg,
                params: cmd.params,
            };
            if let Err(e) = (cmd.handler)(args) {
                println!("{}", e);
            }
        }
    }
}

fn add_dynamic_segment(state_machine: &mut StateMachine, mut state: usize) -> usize {
    state = state_machine.add(state, CharacterSet::from_char(' '));
    let mut char_set = CharacterSet::any();
    char_set.remove(' ');
    state = state_machine.add(state, char_set);
    state_machine.add_next_state(state, state);
    state_machine.start_parse(state);
    state_machine.end_parse(state);

    state
}

fn add_multi_part_dynamic_segment(state_machine: &mut StateMachine, mut state: usize) -> usize {
    state = state_machine.add(state, CharacterSet::from_char(' '));
    state = state_machine.add(state, CharacterSet::from_char('"'));
    state = state_machine.add(state, CharacterSet::any());
    state_machine.add_next_state(state, state);
    state_machine.start_parse(state);
    state_machine.end_parse(state);
    state = state_machine.add(state, CharacterSet::from_char('"'));

    state
}
