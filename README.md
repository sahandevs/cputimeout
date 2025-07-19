# cputimeout

very unsafe way of time-outing a cpu-bounded task.

_(use this crate at your own risk!)_

## Usage

- step 0: don't use this :)
- step 1: but if you have to...

```toml
cputimeout = "*"
```

```rust
use cputimeout::timeout_cpu;

timeout_cpu(
    || loop {},
    std::time::Duration::from_millis(100),
).unwrap();
```

### Nesting

> **state**: just a PoC solution, works up to 10 nested calls

```rust
// inner smaller than outer
let r = timeout_cpu(
    || timeout_cpu(|| loop {}, Duration::from_millis(50)),
    Duration::from_millis(100),
);

assert!(matches!(r, Ok(Err(Error::TimedOut))));

// outer smaller than inner
let r = timeout_cpu(
    || timeout_cpu(|| loop {}, Duration::from_millis(100)),
    Duration::from_millis(50),
);
assert!(matches!(r, Err(Error::TimedOut)));
```

### Allocation tracker

> **state**: just a PoC solution, there are some corner cases.

if the task _timeouts_, this crate does a `jmp` so it may not call `free()` or `Drop` on allocated resources. If you can, allocate everything outside the function and only pass reference to it. If you can't (for example you are using a C library that calls malloc) you can use this feature. this interposes `malloc` calls and tracks all allocations that the task makes and calls `free` on them when timeouts.

just add `mem-tracker` to features list and everything should work.

### TODO

- [ ] follow threads
- [ ] resource tracker
- [ ] tokio support
- [ ] benchmark
- [ ] tests
- [ ] documentation
