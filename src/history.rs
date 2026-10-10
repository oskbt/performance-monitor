//! サンプルの時系列を保持するリングバッファ。

use crate::collector::Snapshot;
use std::collections::VecDeque;
use std::sync::Arc;

pub struct History {
    samples: VecDeque<Arc<Snapshot>>,
    capacity: usize,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        Self { samples: VecDeque::with_capacity(capacity), capacity: capacity.max(2) }
    }

    pub fn push(&mut self, snapshot: Arc<Snapshot>) {
        self.samples.push_back(snapshot);
        self.trim();
    }

    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity.max(2);
        self.trim();
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 古い順のサンプル。
    #[cfg(test)]
    pub fn samples(&self) -> impl ExactSizeIterator<Item = &Arc<Snapshot>> {
        self.samples.iter()
    }

    pub fn latest(&self) -> Option<&Arc<Snapshot>> {
        self.samples.back()
    }

    /// 古い順に各サンプルから値を取り出す。値が無いサンプルは NaN になる。
    pub fn series(&self, f: impl Fn(&Snapshot) -> Option<f32>) -> Vec<f32> {
        self.samples.iter().map(|s| f(s).unwrap_or(f32::NAN)).collect()
    }

    fn trim(&mut self) {
        while self.samples.len() > self.capacity {
            self.samples.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_n(history: &mut History, n: usize) {
        for _ in 0..n {
            history.push(Arc::new(Snapshot::default()));
        }
    }

    #[test]
    fn keeps_only_capacity() {
        let mut history = History::new(3);
        push_n(&mut history, 5);
        assert_eq!(history.samples().len(), 3);
    }

    #[test]
    fn shrinking_capacity_drops_oldest() {
        let mut history = History::new(10);
        push_n(&mut history, 10);
        history.set_capacity(4);
        assert_eq!(history.samples().len(), 4);
        assert_eq!(history.capacity(), 4);
    }

    #[test]
    fn missing_values_are_nan() {
        let mut history = History::new(3);
        push_n(&mut history, 2);
        let series = history.series(|s| s.cpu.as_ref().map(|c| c.usage));
        assert_eq!(series.len(), 2);
        assert!(series.iter().all(|v| v.is_nan()));
    }
}
