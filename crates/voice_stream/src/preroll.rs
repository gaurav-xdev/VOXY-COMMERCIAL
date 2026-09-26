use std::collections::VecDeque;

pub struct PreRollBuffer {
    samples: VecDeque<f32>,
    capacity: usize,
}

impl PreRollBuffer {
    pub fn new(capacity_samples: usize) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity_samples),
            capacity: capacity_samples,
        }
    }

    pub fn push(&mut self, data: &[f32]) {
        if self.capacity == 0 {
            return;
        }
        self.samples.extend(data.iter().copied());
        let overflow = self.samples.len().saturating_sub(self.capacity);
        if overflow > 0 {
            self.samples.drain(..overflow);
        }
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn take(&mut self) -> Vec<f32> {
        self.samples.drain(..).collect()
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

impl Default for PreRollBuffer {
    fn default() -> Self {
        Self::new(24000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_only_the_most_recent_samples() {
        let mut buf = PreRollBuffer::new(4);
        buf.push(&[1.0, 2.0, 3.0]);
        buf.push(&[4.0, 5.0, 6.0]);
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.take(), vec![3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn zero_capacity_holds_nothing() {
        let mut buf = PreRollBuffer::new(0);
        buf.push(&[1.0, 2.0]);
        assert!(buf.is_empty());
    }

    #[test]
    fn take_drains_the_buffer() {
        let mut buf = PreRollBuffer::new(8);
        buf.push(&[1.0, 2.0]);
        assert_eq!(buf.take(), vec![1.0, 2.0]);
        assert!(buf.is_empty());
    }

    #[test]
    fn clear_resets_contents() {
        let mut buf = PreRollBuffer::new(8);
        buf.push(&[1.0, 2.0, 3.0]);
        buf.clear();
        assert!(buf.is_empty());
    }

    #[test]
    fn capacity_is_reported() {
        assert_eq!(PreRollBuffer::new(100).capacity(), 100);
    }
}