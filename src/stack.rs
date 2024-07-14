pub struct Overflow {
    pub other_lost: i32,
    pub self_gain: i32,
}

#[derive(Debug)]
pub struct Stack {
    vec: Vec<i32>,
    len: usize,
}

impl Stack {
    pub fn new(len: usize) -> Self {
        Self { vec: Vec::with_capacity(10), len }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, num: i32) -> Option<Overflow> {
        self.vec.push(num);
        if self.vec.len() == self.len {
           let overflow = Overflow { other_lost: *self.vec.first().unwrap(), self_gain: *self.vec.last().unwrap() };
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

}

impl Default for Stack {
    fn default() -> Self {
        Self::new(10)
    }
}