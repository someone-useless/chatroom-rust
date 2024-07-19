use rand::{
    distributions::{Distribution, Standard, WeightedIndex},
    Rng,
};
use serde::{Deserialize, Serialize};

pub struct Overflow {
    pub other_lost: i32,
    pub self_gain: i32,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Stack {
    vec: Vec<i32>,
    len: usize,
}

impl Stack {
    pub fn new(len: usize) -> Self {
        Self {
            vec: Vec::with_capacity(10),
            len,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, num: i32) -> Option<Overflow> {
        self.vec.push(num);
        if self.vec.len() == self.len {
            let overflow = Overflow {
                other_lost: *self.vec.first().unwrap(),
                self_gain: *self.vec.last().unwrap(),
            };
            self.vec.clear();
            Some(overflow)
        } else {
            None
        }
    }

    pub fn pop(&mut self) {
        self.vec.pop();
    }

    pub fn reverse(&mut self) {
        self.vec.reverse();
    }

    pub fn add(&mut self, num: i32) {
        self.vec.last_mut().map(|n| *n += num);
    }

    pub fn neg(&mut self) {
        self.vec.last_mut().map(|n| *n *= -1);
    }

    pub fn use_action(&mut self, action: &Action) -> Option<Overflow> {
        match action {
          Action::Push(num) => self.push(*num),
          Action::Pop => { self.pop(); None },
          Action::Reverse => { self.reverse(); None },
          Action::Add(num) => { self.add(*num); None },
          Action::Neg => { self.neg(); None },
        }
    }

    pub fn use_card(&mut self, card: &Card) -> Vec<Overflow> {
        card.actions.iter().filter_map(|action| self.use_action(action)).collect()
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new(10)
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Card {
    pub actions: Vec<Action>,
}

pub struct CardDistribution {
    action_number_weight: WeightedIndex<u32>,
    card_weight: WeightedIndex<u32>,
    push_number_weight: WeightedIndex<u32>,
    add_number_weight: WeightedIndex<u32>,
    neg_weight: WeightedIndex<u32>,
}

impl Default for CardDistribution {
    fn default() -> Self {
        Self {
            action_number_weight: WeightedIndex::new([4, 6, 3, 1]).expect("Should success"),
            card_weight: WeightedIndex::new([7, 3, 3, 2, 1]).expect("Should success"),
            push_number_weight: WeightedIndex::new([1, 1, 1, 2, 3, 4, 8, 8, 8, 1])
                .expect("Should success"),
            add_number_weight: WeightedIndex::new([1, 2, 3, 3]).expect("Should success"),
            neg_weight: WeightedIndex::new([1, 7]).expect("Should success"),
        }
    }
}

impl Distribution<Card> for CardDistribution {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Card {
        let action_number = self.action_number_weight.sample(rng) + 1;
        let mut actions = vec![Action::Pop; action_number];
        actions.fill_with(|| {
            let action_type = self.card_weight.sample(rng);
            match action_type {
                0 => {
                    let num = (self.push_number_weight.sample(rng) as i32 - 9)
                        * if self.neg_weight.sample(rng) == 1 {
                            -1
                        } else {
                            1
                        };
                    Action::Push(num)
                }
                1 => Action::Pop,
                2 => Action::Reverse,
                3 => {
                    let num = (self.add_number_weight.sample(rng) as i32 + 1)
                        * if Standard.sample(rng) { -1 } else { 1 };
                    Action::Add(num)
                }
                4 => Action::Neg,
                _ => unreachable!(),
            }
        });

        Card { actions }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "action_type", content = "num", rename_all="snake_case")]
pub enum Action {
    Push(i32),
    Pop,
    Reverse,
    Add(i32),
    Neg,
}