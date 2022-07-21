// Illustration of a case where PhantomData is providing necessary ownership
// info to rustc.
//
// MyBox2<T> uses just a `*const T` to hold the `T` it owns.
// MyBox3<T> has both a `*const T` AND a PhantomData<T>; the latter communicates
// its ownership relationship with `T`.
//
// Skim down to `fn f2()` to see the relevant case, 
// and compare it to `fn f3()`. When you run the program,
// the output will include:
//
// drop PrintOnDrop(mb2b, PrintOnDrop("v2b", 13, INVALID), Valid)
//
// (However, in the absence of #[may_dangle], the compiler will constrain
// things in a manner that may indeed imply that PhantomData is unnecessary;
// pnkfelix is not 100% sure of this claim yet, though.)

#![feature(dropck_eyepatch)]

use std::alloc::{self, dealloc, Layout};
use std::fmt;
use std::marker::PhantomData;
use std::mem;
use std::ptr;

#[derive(Copy, Clone, Debug)]
enum State { INVALID, Valid }

#[derive(Debug)]
struct PrintOnDrop<T: fmt::Debug>(&'static str, T, State);

impl<T: fmt::Debug> PrintOnDrop<T> {
    fn new(name: &'static str, t: T) -> Self {
        PrintOnDrop(name, t, State::Valid)
    }
}

impl<T: fmt::Debug> Drop for PrintOnDrop<T> {
    fn drop(&mut self) {
        println!("drop PrintOnDrop({}, {:?}, {:?})",
                 self.0,
                 self.1,
                 self.2);
        self.2 = State::INVALID;
    }
}

struct MyBox1<T> {
    v: Box<T>,
}

impl<T> MyBox1<T> {
    fn new(t: T) -> Self {
        MyBox1 { v: Box::new(t) }
    }
}

struct MyBox2<T> {
    v: *const T,
}

impl<T> MyBox2<T> {
    fn new(t: T) -> Self {
        unsafe {
            let p = alloc::alloc(Layout::new::<T>());
            let p = p as *mut T;
            ptr::write(p, t);
            MyBox2 { v: p }
        }
    }
}

unsafe impl<#[may_dangle] T> Drop for MyBox2<T> {
    fn drop(&mut self) {
        unsafe {
            // We want this to be *legal*. This destructor is not 
            // allowed to call methods on `T` (since it may be in
            // an invalid state), but it should be allowed to drop
            // instances of `T` as it deconstructs itself.
            //
            // (Note however that the compiler has no knowledge
            //  that `MyBox2<T>` owns an instance of `T`.)
            ptr::read(self.v);
            dealloc(self.v as *mut u8, Layout::new::<T>());
        }
    }
}

struct MyBox3<T> {
    v: *const T,
    _pd: PhantomData<T>,
}

impl<T> MyBox3<T> {
    fn new(t: T) -> Self {
        unsafe {
            let p = alloc::alloc(Layout::new::<T>());
            let p = p as *mut T;
            ptr::write(p, t);
            MyBox3 { v: p, _pd: Default::default() }
        }
    }
}

unsafe impl<#[may_dangle] T> Drop for MyBox3<T> {
    fn drop(&mut self) {
        unsafe {
            ptr::read(self.v);
            dealloc(self.v as *mut u8, Layout::new::<T>());
        }
    }
}

fn f1() {
    // `let (v, _mb1);` and `let (_mb1, v)` won't compile due to dropck
    let v1;
    let _mb1;
    v1 = PrintOnDrop::new("v1", 13);
    _mb1 = MyBox1::new(PrintOnDrop::new("mb1", &v1));
}

fn f2() {
    {
        let (v2a, _mb2a); // Sound, but not distinguished from below by rustc!
        v2a = PrintOnDrop::new("v2a", 13);
        _mb2a = MyBox2::new(PrintOnDrop::new("mb2a", &v2a));
    }

    {
        let (_mb2b, v2b); // Unsound!
        v2b = PrintOnDrop::new("v2b", 13);
        _mb2b = MyBox2::new(PrintOnDrop::new("mb2b", &v2b));
        // namely, v2b dropped before _mb2b, but latter contains
        // value that attempts to access v2b when being dropped.
    }
}

fn f3() {
    let v3;
    let _mb3; // `let (v, mb3);` won't compile due to dropck
    v3 = PrintOnDrop::new("v3", 13);
    _mb3 = MyBox3::new(PrintOnDrop::new("mb3", &v3));
}

fn main() {
    f1();
    f2();
    f3();
}