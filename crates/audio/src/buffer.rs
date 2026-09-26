use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::error::{AudioError, Result};

/// Lock-free SPSC (Single-Producer Single-Consumer) ring buffer.
///
/// The producer calls `write()`, the consumer calls `read()`.
/// Only safe to use from one thread writing and one thread reading concurrently.
pub struct SpscRingBuffer {
    buf: UnsafeCell<Vec<f32>>,
    write_pos: AtomicUsize,
    read_pos: AtomicUsize,
    capacity: usize,
    closed: AtomicBool,
}

// SAFETY: The SPSC contract guarantees that only one thread calls write()
// and one thread calls read(), so there is no aliased mutable access.
unsafe impl Send for SpscRingBuffer {}
unsafe impl Sync for SpscRingBuffer {}

impl SpscRingBuffer {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        assert!(
            capacity.is_power_of_two(),
            "capacity must be a power of two"
        );
        Self {
            buf: UnsafeCell::new(vec![0.0; capacity]),
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
            capacity,
            closed: AtomicBool::new(false),
        }
    }

    #[inline]
    #[allow(dead_code)]
    fn mask(&self, pos: usize) -> usize {
        pos & (self.capacity - 1)
    }

    /// Available samples for the consumer to read.
    #[inline]
    pub fn len(&self) -> usize {
        let w = self.write_pos.load(Ordering::Acquire);
        let r = self.read_pos.load(Ordering::Acquire);
        w.wrapping_sub(r)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Available slots for the producer to write.
    #[inline]
    pub fn spare(&self) -> usize {
        self.capacity - self.len()
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.spare() == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Producer: write samples into the buffer. Returns number of samples actually written.
    pub fn write(&self, data: &[f32]) -> usize {
        let avail = self.spare();
        let to_write = data.len().min(avail);
        if to_write == 0 {
            return 0;
        }

        let wp = self.write_pos.load(Ordering::Relaxed);
        let mask = self.capacity - 1;
        let base = wp & mask;

        // SAFETY: Only the producer thread calls this method, so there is no aliasing.
        let buf = unsafe { &mut *self.buf.get() };

        let first_chunk = (self.capacity - base).min(to_write);
        buf[base..base + first_chunk].copy_from_slice(&data[..first_chunk]);
        if to_write > first_chunk {
            let second_chunk = to_write - first_chunk;
            buf[..second_chunk].copy_from_slice(&data[first_chunk..first_chunk + second_chunk]);
        }

        self.write_pos
            .store(wp.wrapping_add(to_write), Ordering::Release);
        to_write
    }

    /// Consumer: read samples from the buffer. Returns number of samples actually read.
    pub fn read(&self, output: &mut [f32]) -> usize {
        let avail = self.len();
        let to_read = output.len().min(avail);
        if to_read == 0 {
            return 0;
        }

        let rp = self.read_pos.load(Ordering::Relaxed);
        let mask = self.capacity - 1;
        let base = rp & mask;

        // SAFETY: Only the consumer thread calls this method, so there is no aliasing.
        let buf = unsafe { &*self.buf.get() };

        let first_chunk = (self.capacity - base).min(to_read);
        output[..first_chunk].copy_from_slice(&buf[base..base + first_chunk]);
        if to_read > first_chunk {
            let second_chunk = to_read - first_chunk;
            output[first_chunk..first_chunk + second_chunk].copy_from_slice(&buf[..second_chunk]);
        }

        self.read_pos
            .store(rp.wrapping_add(to_read), Ordering::Release);
        to_read
    }

    /// Discard all buffered data.
    pub fn clear(&self) {
        self.read_pos
            .store(self.write_pos.load(Ordering::Acquire), Ordering::Release);
    }

    /// Signal the consumer that no more data will be written.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod spsc_tests {
    use super::*;

    #[test]
    fn test_spsc_new() {
        let rb = SpscRingBuffer::new(1024);
        assert_eq!(rb.capacity(), 1024);
        assert!(rb.is_empty());
        assert!(!rb.is_full());
    }

    #[test]
    fn test_spsc_write_read() {
        let rb = SpscRingBuffer::new(1024);
        let written = rb.write(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(written, 4);
        assert_eq!(rb.len(), 4);

        let mut out = [0.0f32; 4];
        let read = rb.read(&mut out);
        assert_eq!(read, 4);
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
        assert!(rb.is_empty());
    }

    #[test]
    fn test_spsc_wrap_around() {
        let rb = SpscRingBuffer::new(8);
        rb.write(&[1.0, 2.0, 3.0, 4.0]);
        let mut out = [0.0f32; 4];
        rb.read(&mut out);

        let written = rb.write(&[5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
        assert_eq!(written, 6);

        let mut out2 = [0.0f32; 6];
        let read = rb.read(&mut out2);
        assert_eq!(read, 6);
        assert_eq!(out2, [5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
    }

    #[test]
    fn test_spsc_full_buffer() {
        let rb = SpscRingBuffer::new(4);
        let written = rb.write(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(written, 4);
        assert!(rb.is_full());
        let extra = rb.write(&[5.0]);
        assert_eq!(extra, 0);
    }

    #[test]
    fn test_spsc_empty_read() {
        let rb = SpscRingBuffer::new(4);
        let mut out = [0.0f32; 1];
        let read = rb.read(&mut out);
        assert_eq!(read, 0);
    }

    #[test]
    fn test_spsc_partial_read() {
        let rb = SpscRingBuffer::new(8);
        rb.write(&[1.0, 2.0, 3.0, 4.0, 5.0]);

        let mut out = [0.0f32; 3];
        let read = rb.read(&mut out);
        assert_eq!(read, 3);
        assert_eq!(out, [1.0, 2.0, 3.0]);

        let mut out2 = [0.0f32; 3];
        let read2 = rb.read(&mut out2);
        assert_eq!(read2, 2);
        assert_eq!(out2[..2], [4.0, 5.0]);
    }

    #[test]
    fn test_spsc_partial_write() {
        let rb = SpscRingBuffer::new(4);
        rb.write(&[1.0, 2.0]);

        let written = rb.write(&[3.0, 4.0, 5.0]);
        assert_eq!(written, 2);

        let mut out = [0.0f32; 4];
        rb.read(&mut out);
        assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_spsc_multiple_cycles() {
        let rb = SpscRingBuffer::new(4);
        for _ in 0..10 {
            rb.write(&[1.0, 2.0]);
            let mut out = [0.0f32; 2];
            rb.read(&mut out);
            assert_eq!(out, [1.0, 2.0]);
        }
    }

    #[test]
    fn test_spsc_clear() {
        let rb = SpscRingBuffer::new(8);
        rb.write(&[1.0, 2.0, 3.0]);
        assert!(!rb.is_empty());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn test_spsc_close() {
        let rb = SpscRingBuffer::new(4);
        assert!(!rb.is_closed());
        rb.close();
        assert!(rb.is_closed());
    }

    #[test]
    fn test_spsc_spare() {
        let rb = SpscRingBuffer::new(8);
        assert_eq!(rb.spare(), 8);
        rb.write(&[1.0, 2.0, 3.0]);
        assert_eq!(rb.spare(), 5);
    }

    #[test]
    fn test_spsc_zero_write() {
        let rb = SpscRingBuffer::new(4);
        let written = rb.write(&[]);
        assert_eq!(written, 0);
    }

    #[test]
    fn test_spsc_zero_read() {
        let rb = SpscRingBuffer::new(4);
        rb.write(&[1.0]);
        let mut out = [];
        let read = rb.read(&mut out);
        assert_eq!(read, 0);
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn test_spsc_zero_capacity() {
        SpscRingBuffer::new(0);
    }

    #[test]
    #[should_panic(expected = "capacity must be a power of two")]
    fn test_spsc_non_power_of_two() {
        SpscRingBuffer::new(7);
    }
}

pub struct RingBuffer {
    buffer: Vec<f32>,
    write_pos: AtomicUsize,
    read_pos: AtomicUsize,
    capacity: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be greater than 0");
        assert!(
            capacity.is_power_of_two(),
            "capacity must be a power of two"
        );
        Self {
            buffer: vec![0.0; capacity],
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
            capacity,
        }
    }

    fn mask(&self, pos: usize) -> usize {
        pos & (self.capacity - 1)
    }

    pub fn write(&mut self, data: &[f32]) -> Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }

        let avail = self.available_for_write();
        let to_write = data.len().min(avail);

        if to_write == 0 {
            return Err(AudioError::BufferOverflow);
        }

        let write_pos = self.write_pos.load(Ordering::Relaxed);

        for (i, &sample) in data.iter().enumerate().take(to_write) {
            let idx = self.mask(write_pos + i);
            self.buffer[idx] = sample;
        }

        self.write_pos
            .store(write_pos + to_write, Ordering::Release);

        Ok(to_write)
    }

    pub fn read(&mut self, data: &mut [f32]) -> Result<usize> {
        if data.is_empty() {
            return Ok(0);
        }

        let avail = self.available_for_read();
        let to_read = data.len().min(avail);

        if to_read == 0 {
            return Err(AudioError::BufferUnderflow);
        }

        let read_pos = self.read_pos.load(Ordering::Relaxed);

        for (i, dest) in data.iter_mut().enumerate().take(to_read) {
            let idx = self.mask(read_pos + i);
            *dest = self.buffer[idx];
        }

        self.read_pos.store(read_pos + to_read, Ordering::Release);

        Ok(to_read)
    }

    pub fn available_for_read(&self) -> usize {
        let write_pos = self.write_pos.load(Ordering::Acquire);
        let read_pos = self.read_pos.load(Ordering::Acquire);
        write_pos - read_pos
    }

    pub fn available_for_write(&self) -> usize {
        self.capacity - self.available_for_read()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.read_pos.store(0, Ordering::Release);
        self.write_pos.store(0, Ordering::Release);
    }

    pub fn is_empty(&self) -> bool {
        self.available_for_read() == 0
    }

    pub fn is_full(&self) -> bool {
        self.available_for_write() == 0
    }
}

pub struct AudioBufferPool {
    buffers: Vec<Vec<f32>>,
    in_use: Vec<bool>,
    #[allow(dead_code)]
    buffer_size: usize,
}

impl AudioBufferPool {
    pub fn new(count: usize, buffer_size: usize) -> Self {
        let mut buffers = Vec::with_capacity(count);
        for _ in 0..count {
            buffers.push(vec![0.0; buffer_size]);
        }
        Self {
            buffers,
            in_use: vec![false; count],
            buffer_size,
        }
    }

    pub fn acquire(&mut self) -> Option<Vec<f32>> {
        for (i, in_use) in self.in_use.iter_mut().enumerate() {
            if !*in_use {
                *in_use = true;
                let buf = self.buffers[i].clone();
                return Some(buf);
            }
        }
        None
    }

    pub fn release(&mut self, buffer: Vec<f32>) {
        for (i, b) in self.buffers.iter_mut().enumerate() {
            if b.len() == buffer.len() && self.in_use[i] {
                *b = buffer;
                self.in_use[i] = false;
                return;
            }
        }
    }

    pub fn available(&self) -> usize {
        self.in_use.iter().filter(|&&in_use| !in_use).count()
    }

    pub fn total(&self) -> usize {
        self.buffers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buffer_new() {
        let rb = RingBuffer::new(1024);
        assert_eq!(rb.capacity(), 1024);
        assert!(rb.is_empty());
        assert!(!rb.is_full());
    }

    #[test]
    fn test_ring_buffer_write_read() {
        let mut rb = RingBuffer::new(1024);
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let written = rb.write(&data).unwrap();
        assert_eq!(written, 4);

        let mut out = vec![0.0; 4];
        let read = rb.read(&mut out).unwrap();
        assert_eq!(read, 4);
        assert_eq!(out, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_ring_buffer_wrap_around() {
        let mut rb = RingBuffer::new(8);
        let data = vec![1.0, 2.0, 3.0, 4.0];
        rb.write(&data).unwrap();
        let mut out = vec![0.0; 4];
        rb.read(&mut out).unwrap();

        let data2 = vec![5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        let written = rb.write(&data2).unwrap();
        assert_eq!(written, 6);

        let mut out2 = vec![0.0; 6];
        let read = rb.read(&mut out2).unwrap();
        assert_eq!(read, 6);
        assert_eq!(out2, vec![5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let mut rb = RingBuffer::new(4);
        let data = vec![1.0, 2.0, 3.0, 4.0];
        rb.write(&data).unwrap();
        let result = rb.write(&[5.0]);
        assert!(result.is_err());
    }

    #[test]
    fn test_ring_buffer_underflow() {
        let mut rb = RingBuffer::new(4);
        let mut out = vec![0.0; 1];
        let result = rb.read(&mut out);
        assert!(result.is_err());
    }

    #[test]
    fn test_ring_buffer_empty_full() {
        let mut rb = RingBuffer::new(4);
        assert!(rb.is_empty());
        assert!(!rb.is_full());

        rb.write(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        assert!(!rb.is_empty());
        assert!(rb.is_full());
    }

    #[test]
    fn test_ring_buffer_clear() {
        let mut rb = RingBuffer::new(8);
        rb.write(&[1.0, 2.0, 3.0]).unwrap();
        assert!(!rb.is_empty());
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.available_for_read(), 0);
    }

    #[test]
    fn test_ring_buffer_partial_read() {
        let mut rb = RingBuffer::new(8);
        rb.write(&[1.0, 2.0, 3.0, 4.0, 5.0]).unwrap();

        let mut out = vec![0.0; 3];
        let read = rb.read(&mut out).unwrap();
        assert_eq!(read, 3);
        assert_eq!(out, vec![1.0, 2.0, 3.0]);

        let mut out2 = vec![0.0; 3];
        let read2 = rb.read(&mut out2).unwrap();
        assert_eq!(read2, 2);
        assert_eq!(out2[..2], vec![4.0, 5.0]);
    }

    #[test]
    fn test_ring_buffer_partial_write() {
        let mut rb = RingBuffer::new(4);
        rb.write(&[1.0, 2.0]).unwrap();

        let written = rb.write(&[3.0, 4.0, 5.0]).unwrap();
        assert_eq!(written, 2);

        let mut out = vec![0.0; 4];
        rb.read(&mut out).unwrap();
        assert_eq!(out, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_ring_buffer_multiple_wraps() {
        let mut rb = RingBuffer::new(4);
        for cycle in 0..10 {
            rb.write(&[1.0, 2.0]).unwrap();
            let mut out = vec![0.0; 2];
            rb.read(&mut out).unwrap();
            assert_eq!(out, vec![1.0, 2.0], "cycle {cycle}");
        }
    }

    #[test]
    fn test_ring_buffer_zero_write() {
        let mut rb = RingBuffer::new(4);
        let written = rb.write(&[]).unwrap();
        assert_eq!(written, 0);
    }

    #[test]
    fn test_ring_buffer_zero_read() {
        let mut rb = RingBuffer::new(4);
        rb.write(&[1.0]).unwrap();
        let mut out = vec![];
        let read = rb.read(&mut out).unwrap();
        assert_eq!(read, 0);
    }

    #[test]
    fn test_audio_buffer_pool_new() {
        let pool = AudioBufferPool::new(4, 1024);
        assert_eq!(pool.total(), 4);
        assert_eq!(pool.available(), 4);
    }

    #[test]
    fn test_audio_buffer_pool_acquire_release() {
        let mut pool = AudioBufferPool::new(2, 256);
        let buf = pool.acquire().unwrap();
        assert_eq!(buf.len(), 256);
        assert_eq!(pool.available(), 1);

        pool.release(buf);
        assert_eq!(pool.available(), 2);
    }

    #[test]
    fn test_audio_buffer_pool_exhaustion() {
        let mut pool = AudioBufferPool::new(2, 64);
        let _b1 = pool.acquire().unwrap();
        let _b2 = pool.acquire().unwrap();
        assert!(pool.acquire().is_none());
        assert_eq!(pool.available(), 0);
    }

    #[test]
    fn test_audio_buffer_pool_available_count() {
        let mut pool = AudioBufferPool::new(3, 512);
        assert_eq!(pool.available(), 3);

        let _b1 = pool.acquire().unwrap();
        assert_eq!(pool.available(), 2);

        let _b2 = pool.acquire().unwrap();
        assert_eq!(pool.available(), 1);

        let _b3 = pool.acquire().unwrap();
        assert_eq!(pool.available(), 0);
    }

    #[test]
    #[should_panic(expected = "capacity must be greater than 0")]
    fn test_ring_buffer_zero_capacity() {
        RingBuffer::new(0);
    }

    #[test]
    fn test_ring_buffer_available_counts() {
        let mut rb = RingBuffer::new(8);
        assert_eq!(rb.available_for_read(), 0);
        assert_eq!(rb.available_for_write(), 8);

        rb.write(&[1.0, 2.0, 3.0]).unwrap();
        assert_eq!(rb.available_for_read(), 3);
        assert_eq!(rb.available_for_write(), 5);
    }
}
