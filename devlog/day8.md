# Day 8

## Having Multiple Owners of Mutable Data by Combining Rc<T> and RefCell<T>

> A common way to use RefCell<T> is in combination with Rc<T>. Recall that Rc<T> lets you have multiple owners of some data, but it only gives immutable access to that data. If you have an Rc<T> that holds a RefCell<T>, you can get a value that can have multiple owners and that you can mutate!

https://doc.rust-lang.org/book/ch15-05-interior-mutability.html#having-multiple-owners-of-mutable-data-by-combining-rct-and-refcellt

```rust
#[derive(Debug)]
enum List {
    Cons(Rc<RefCell<i32>>, Rc<List>),
    Nil,
}

use List::{Cons, Nil};
use std::rc::Rc;
use std::cell::RefCell;

fn main() {
    let value = Rc::new(RefCell::new(5));

    let a = Rc::new(Cons(Rc::clone(&value), Rc::new(Nil)));

    let b = Cons(Rc::new(RefCell::new(6)), Rc::clone(&a));
    let c = Cons(Rc::new(RefCell::new(10)), Rc::clone(&a));

    *value.borrow_mut() += 10;

    println!("a after = {:?}", a);
    println!("b after = {:?}", b);
    println!("c after = {:?}", c);
}
```


> The standard library has other types that provide interior mutability, such as Cell<T>, which is similar except that instead of giving references to the inner value, the value is copied in and out of the Cell<T>. There’s also Mutex<T>, which offers interior mutability that’s safe to use across threads; we’ll discuss its use in Chapter 16. Check out the standard library docs for more details on the differences between these types.

## Weak References

> So far, we’ve demonstrated that calling Rc::clone increases the strong_count of an Rc<T> instance, and an Rc<T> instance is only cleaned up if its strong_count is 0. You can also create a weak reference to the value within an Rc<T> instance by calling Rc::downgrade and passing a reference to the Rc<T>. When you call Rc::downgrade, you get a smart pointer of type Weak<T>. Instead of increasing the strong_count in the Rc<T> instance by 1, calling Rc::downgrade increases the weak_count by 1. The Rc<T> type uses weak_count to keep track of how many Weak<T> references exist, similar to strong_count. The difference is the weak_count doesn’t need to be 0 for the Rc<T> instance to be cleaned up.
> 
> Strong references are how you can share ownership of an Rc<T> instance. Weak references don’t express an ownership relationship. They won’t cause a reference cycle because any cycle involving some weak references will be broken once the strong reference count of values involved is 0.
>
> Because the value that Weak<T> references might have been dropped, to do anything with the value that a Weak<T> is pointing to, you must make sure the value still exists. Do this by calling the upgrade method on a Weak<T> instance, which will return an Option<Rc<T>>. You’ll get a result of Some if the Rc<T> value has not been dropped yet and a result of None if the Rc<T> value has been dropped. Because upgrade returns an Option<T>, Rust will ensure that the Some case and the None case are handled, and there won’t be an invalid pointer.
>
> Thinking about the relationships another way, a parent node should own its children: if a parent node is dropped, its child nodes should be dropped as well. However, a child should not own its parent: if we drop a child node, the parent should still exist. This is a case for weak references!

## summary

> This chapter covered how to use smart pointers to make different guarantees and trade-offs than those Rust makes by default with regular references. The Box<T> type has a known size and points to data allocated on the heap. The Rc<T> type keeps track of the number of references to data on the heap so that data can have multiple owners. The RefCell<T> type with its interior mutability gives us a type that we can use when we need an immutable type but need to change an inner value of that type; it also enforces the borrowing rules at runtime instead of at compile time.
>
> If this chapter has piqued your interest and you want to implement your own smart pointers, check out “The Rustonomicon” for more useful information.

https://doc.rust-lang.org/stable/nomicon/

## Atomic Reference Counting

> Fortunately, Arc<T> is a type like Rc<T> that is safe to use in concurrent situations. The a stands for atomic, meaning it’s an atomically reference counted type. Atomics are an additional kind of concurrency primitive that we won’t cover in detail here: see the standard library documentation for std::sync::atomic for more details. At this point, you just need to know that atomics work like primitive types but are safe to share across threads.

```rust
use std::sync::{Mutex, Arc};
use std::thread;

fn main() {
    let counter = Arc::new(Mutex::new(0));
    let mut handles = vec![];

    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            let mut num = counter.lock().unwrap();

            *num += 1;
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    println!("Result: {}", *counter.lock().unwrap());
}
```

Good tutorials from this guy https://falseidolfactory.com/2018/08/16/gfx-hal-part-0-drawing-a-triangle.html