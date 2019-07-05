use crate::commands::CmdPtr;
use std::{collections::HashMap, u64};

/// # CharacterSet
///
/// Stores the characters for a character set
#[derive(Eq, PartialEq, Clone)]
pub(crate) struct CharacterSet {
    low_mask: u64,
    high_mask: u64,
}

impl CharacterSet {
    pub(crate) fn new() -> Self {
        Self {
            low_mask: 0,
            high_mask: 0,
        }
    }

    pub(crate) fn any() -> Self {
        Self {
            low_mask: u64::MAX,
            high_mask: u64::MAX,
        }
    }

    /// Add a character to the character set.   
    pub(crate) fn insert(&mut self, ch: char) {
        let val = ch as u32 - 1;

        match val {
            0..=63 => {
                let bit = 1 << val;
                self.low_mask = self.low_mask | bit;
            }
            64..=127 => {
                let bit = 1 << val - 64;
                self.high_mask = self.high_mask | bit;
            }
            _ => {}
        }
    }

    /// Remove a character from the character set.   
    pub(crate) fn remove(&mut self, ch: char) {
        let val = ch as u32 - 1;

        match val {
            0..=63 => {
                let bit = 1 << val;
                self.low_mask = self.low_mask & !bit;
            }
            64..=127 => {
                let bit = 1 << val - 64;
                self.high_mask = self.high_mask & !bit;
            }
            _ => {}
        }
    }

    /// Check if the character `ch` is a member of the character set.  
    pub(crate) fn contains(&self, ch: char) -> bool {
        let val = ch as u32 - 1;

        match val {
            0..=63 => {
                let bit = 1 << val;
                self.low_mask & bit != 0
            }
            64..=127 => {
                // flip a bit within 0 - 63
                let bit = 1 << val - 64;
                self.high_mask & bit != 0
            }
            _ => false,
        }
    }

    /// Insert the character `ch` into the character set.  
    pub(crate) fn from_char(ch: char) -> Self {
        let mut chars = Self::new();
        chars.insert(ch);
        chars
    }
}

pub(crate) struct State {
    index: usize,
    expected: CharacterSet,
    next_states: Vec<usize>,
    is_final_state: bool,
    handler: Option<CmdPtr>,
    param_names: Option<Vec<&'static str>>,
}

impl PartialEq for State {
    fn eq(&self, other: &State) -> bool {
        self.index == other.index
    }
}

impl State {
    pub(crate) fn new(index: usize, expected: CharacterSet) -> Self {
        Self {
            index,
            expected,
            next_states: Vec::new(),
            is_final_state: false,
            handler: None,
            param_names: None,
        }
    }
}

/// # Traversal
#[derive(Clone)]
pub(crate) struct Traversal {
    current_state: usize,
    positions: Vec<(usize, usize)>,
    segment_start: Option<usize>,
}

impl Traversal {
    /// Create a new traversal.  
    pub(crate) fn new() -> Self {
        Self {
            current_state: 0,
            positions: Vec::new(),
            segment_start: None,
        }
    }

    /// Mark the position in the input where a dynamic segment begins.  
    pub(crate) fn set_segment_start(&mut self, pos: usize) {
        self.segment_start = Some(pos);
    }

    /// Mark the position in the input where a dynamic segment ends.   
    pub(crate) fn set_segment_end(&mut self, pos: usize) {
        self.positions.push((self.segment_start.unwrap(), pos));
        self.segment_start = None;
    }

    /// Returns a vector of the dynamic segments parsed from the input.  
    pub(crate) fn extract<'a>(&self, input: &'a str) -> Vec<&'a str> {
        self.positions
            .iter()
            .map(|&(start, end)| &input[start..end])
            .collect()
    }
}

pub(crate) struct Match<'m> {
    pub handler: &'m CmdPtr,
    pub params: HashMap<&'m str, &'m str>,
}

pub(crate) struct StateMachine {
    states: Vec<State>,
    start_parse: Vec<bool>,
    end_parse: Vec<bool>,
}

impl StateMachine {
    pub(crate) fn new() -> Self {
        Self {
            states: vec![State::new(0, CharacterSet::new())],
            start_parse: vec![false],
            end_parse: vec![false],
        }
    }

    /// Add a state to the state machine.  
    pub(crate) fn add(&mut self, index: usize, expected: CharacterSet) -> usize {
        for &next_index in &self.states[index].next_states {
            let state = &self.states[next_index];
            if state.expected == expected {
                return next_index;
            }
        }

        let state = self.new_state(expected);
        self.states[index].next_states.push(state);
        state
    }

    /// Add a next state to the next_states of an existing state in the state machine.  
    pub(crate) fn add_next_state(&mut self, index: usize, next_index: usize) {
        let next_states = &mut self.states[index].next_states;

        if !next_states.contains(&next_index) {
            next_states.push(next_index);
        }
    }

    fn new_state(&mut self, expected: CharacterSet) -> usize {
        let index = self.states.len();
        let state = State::new(index, expected);

        self.states.push(state);
        self.start_parse.push(false);
        self.end_parse.push(false);
        index
    }

    /// Set the `is_final_state` flag on a state to true.  
    pub(crate) fn set_final_state(&mut self, index: usize) {
        self.states[index].is_final_state = true;
    }

    /// Set the handler function for a state.  
    pub(crate) fn set_handler(&mut self, index: usize, handler: CmdPtr) {
        let state = &mut self.states[index];
        state.handler = Some(handler);
    }

    /// Set the expected parameter keys for the params map.  
    pub(crate) fn set_param_names(&mut self, index: usize, names: Vec<&'static str>) {
        let state = &mut self.states[index];
        state.param_names = Some(names);
    }

    /// Mark that the index in the state machine is a state to start parsing a dynamic
    /// segment.  
    pub(crate) fn start_parse(&mut self, index: usize) {
        self.start_parse[index] = true;
    }

    /// Mark that the index in the state machine is a state to stop parsing a dynamic
    /// segment.  
    pub(crate) fn end_parse(&mut self, index: usize) {
        self.end_parse[index] = true;
    }

    /// Run the input through the state machine, optionally returning a handler and params.  
    pub(crate) fn process<'m>(&'m self, input: &'m str) -> Option<Match<'m>> {
        let mut traversals = vec![Traversal::new()];

        for (i, ch) in input.chars().enumerate() {
            let next_traversals = self.process_char(traversals, ch, i);
            traversals = next_traversals;

            if traversals.is_empty() {
                return None;
            }
        }

        let traversals = traversals
            .into_iter()
            .filter(|traversal| self.states[traversal.current_state].is_final_state)
            .map(|mut traversal| {
                if traversal.segment_start.is_some() {
                    traversal.set_segment_end(input.len());
                }
                traversal
            })
            .collect::<Vec<Traversal>>();

        if traversals.is_empty() {
            None
        } else {
            let traversal = &traversals[0];
            let state = &self.states[traversal.current_state];
            let mut params = HashMap::new();

            if let Some(ref param_names) = state.param_names {
                param_names
                    .iter()
                    .zip(traversal.extract(input))
                    .for_each(|(key, value)| {
                        params.insert(*key, value);
                    });
            }

            Some({
                Match {
                    handler: state.handler.as_ref().unwrap(),
                    params,
                }
            })
        }
    }

    fn process_char(&self, traversals: Vec<Traversal>, ch: char, pos: usize) -> Vec<Traversal> {
        let mut ret = Vec::with_capacity(traversals.len());

        for mut traversal in traversals.into_iter() {
            let current_state = &self.states[traversal.current_state];

            let mut count = 0;
            let mut state_index = 0;

            current_state.next_states.iter().for_each(|index| {
                let next_state = &self.states[*index];

                if next_state.expected.contains(ch) {
                    count += 1;
                    state_index = *index;
                }
            });

            if count == 1 {
                traversal.current_state = state_index;
                self.extract_parse_info(&mut traversal, current_state.index, state_index, pos);
                ret.push(traversal);
                continue;
            }

            current_state.next_states.iter().for_each(|index| {
                let next_state = &self.states[*index];

                if next_state.expected.contains(ch) {
                    let mut copy = traversal.clone();
                    copy.current_state = next_state.index;
                    self.extract_parse_info(&mut copy, current_state.index, *index, pos);
                    ret.push(copy);
                }
            });
        }
        ret
    }

    fn extract_parse_info(
        &self,
        traversal: &mut Traversal,
        current_state: usize,
        next_state: usize,
        pos: usize,
    ) {
        if traversal.segment_start.is_none() && self.start_parse[next_state] {
            traversal.set_segment_start(pos);
        }
        if traversal.segment_start.is_some()
            && self.end_parse[current_state]
            && current_state < next_state
        {
            traversal.set_segment_end(pos);
        }
    }
}
