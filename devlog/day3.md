# Day 3

Read up on Copy trait: https://doc.rust-lang.org/std/marker/trait.Copy.html
the section on implementing manually sounded interesting. I bet it can be combined with
lifetimes (a feature I haven't read about yet) to be very memory efficient in some circumstances.

Hardest thing to accept so far was in Ch. 4:

> In addition, there’s a design choice that’s implied by this: Rust will never automatically create “deep” copies of your data. Therefore, any automatic copying can be assumed to be inexpensive in terms of runtime performance.

https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html#the-stack-and-the-heap

Cool how much Rust pushes away from runtime checking and puts in the compiler.