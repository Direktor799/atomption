use std::marker::PhantomData;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicPtr, Ordering};

type PhantomUnsync<T> = PhantomData<*mut T>;

pub struct AtomicOption<T> {
    inner: AtomicPtr<T>,
    _phantom: PhantomUnsync<T>,
}

impl<T> AtomicOption<T> {
    #[inline(always)]
    pub fn new(data: Option<Box<T>>) -> AtomicOption<T> {
        let empty = AtomicOption {
            inner: AtomicPtr::new(null_mut()),
            _phantom: PhantomData,
        };
        empty.store(data);
        empty
    }

    #[inline(always)]
    pub fn swap(&self, new: Option<Box<T>>) -> Option<Box<T>> {
        let addr = if let Some(new) = new {
            Box::into_raw(new)
        } else {
            null_mut()
        };

        let addr = self.inner.swap(addr, Ordering::AcqRel);
        if addr.is_null() {
            None
        } else {
            Some(unsafe { Box::from_raw(addr) })
        }
    }

    #[inline(always)]
    pub fn take(&self) -> Option<Box<T>> {
        self.swap(None)
    }

    #[inline(always)]
    pub fn store(&self, new: Option<Box<T>>) {
        drop(self.swap(new))
    }
}

unsafe impl<T> Sync for AtomicOption<T> where T: Send {}
unsafe impl<T> Send for AtomicOption<T> where T: Send {}

impl<T> Drop for AtomicOption<T> {
    fn drop(&mut self) {
        let _ = self.take();
    }
}

#[cfg(test)]
mod tests {
    use std::{mem::transmute, thread};

    use super::AtomicOption;

    #[test]
    fn test_simple() {
        let opt = AtomicOption::new(None);
        assert_eq!(opt.take(), None);
        assert_eq!(opt.swap(Some(Box::new(0))), None);
        assert_eq!(opt.take(), Some(Box::new(0)));
        opt.store(Some(Box::new(1)));
        opt.store(Some(Box::new(2)));
        assert_eq!(opt.swap(Some(Box::new(3))), Some(Box::new(2)));
    }

    #[test]
    fn test_two_threads() {
        for _ in 0..100 {
            let opt = AtomicOption::<i64>::new(None);
            let opt: &'static AtomicOption<i64> = unsafe { transmute(&opt) };
            let func1 = move || {
                let mut remain = 100;
                loop {
                    let a = opt.swap(Some(Box::new(remain)));
                    if a.is_none() {
                        remain -= 1;
                    }
                    if remain == 0 {
                        break;
                    }
                }
            };

            let func2 = move || {
                let mut remain = 100;
                loop {
                    let a = opt.swap(None);
                    if a.is_some() {
                        remain -= 1;
                    }
                    if remain == 0 {
                        break;
                    }
                }
            };

            for h in [thread::spawn(func1), thread::spawn(func2)] {
                h.join().unwrap();
            }
            assert_eq!(opt.take(), None);
        }
    }
}
