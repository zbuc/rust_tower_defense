# Day 2

blooog

https://doc.rust-lang.org/book/ch08-03-hash-maps.html#hashing-functions

> By default, HashMap uses a “cryptographically strong”1 hashing function that can provide resistance to Denial of Service (DoS) attacks. This is not the fastest hashing algorithm available, but the trade-off for better security that comes with the drop in performance is worth it. If you profile your code and find that the default hash function is too slow for your purposes, you can switch to another function by specifying a different hasher. A hasher is a type that implements the BuildHasher trait. We’ll talk about traits and how to implement them in Chapter 10. You don’t necessarily have to implement your own hasher from scratch; crates.io has libraries shared by other Rust users that provide hashers implementing many common hashing algorithms.

Maybe I should use a faster hashing algorithm, there is no need for the HashMap implementation for a game to be cryptographically strong for most purposes, except _perhaps_ anti-cheat.

On to Chapter 9 of the book. Itching to write unit tests.

Split code into separate modules. Pretty sure the structs and functions right now will all be useless for the final product,
but you gotta start somewhere.

Got JSON map loading working. The way import visibility works in Rust is difficult to get my head around as a beginner.