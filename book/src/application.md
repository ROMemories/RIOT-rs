# Building an Application

## Obtaining Peripheral Access

Embassy defines a type for each MCU peripheral, which needs to be provided to the driver of that peripheral.
These peripheral types, which we call *Embassy peripherals* or *peripheral ZSTs*, are [Zero Sized Types](https://doc.rust-lang.org/nomicon/exotic-sizes.html#zero-sized-types-zsts) (ZSTs) that are used to statistically enforce exclusive access to a peripheral.
These ZSTs indeed are by design neither [`Copy`](https://doc.rust-lang.org/stable/std/marker/trait.Copy.html) nor [`Clone`](https://doc.rust-lang.org/stable/std/clone/trait.Clone.html), making it impossible to duplicate them; they can only be *move*d around.

Drivers therefore require such ZSTs to be provided to make sure that the caller has (a) access to the peripheral and (b) is the only one having access, since only instance of the type can exist at any time.
Being ZSTs, they do not carry any data to the drivers, only their ownership is meaningful, which is enforced by taking them as parameters for drivers.

> If you are used to thinking about MCU peripherals as referenced by a base address (in the case of memory-mapped peripherals), you can think of these ZSTs as abstraction over these, with a zero-cost, statically-enforced lock ensuring exclusive access.

These Embassy types are defined by [Embassy architecture crates][embassy-architecture-crates] in the respective `peripherals` modules.
In RIOT-rs applications, the only safe way to obtain an instance of an Embassy peripheral is by using the [`define_peripherals!`][define_peripherals-docs] and (optionally) the [`group_peripherals!`][group_peripherals-docs] macros, combined with a [spawner or task][spawner-or-task].

### Example

The [`define_peripherals!`][define_peripherals-docs] macro allows to define a *RIOT-rs peripheral struct*, an instance of which can be obtained with [`spawner` or `task`][spawner-or-task]:

```rust,ignore
{{ #include ../../examples/blinky/src/pins.rs:nrf52840dk-define_peripherals }}
```

Multiple RIOT-rs peripheral structs can be grouped into another RIOT-rs peripheral struct using the [`group_peripherals!`][group_peripherals-docs] macro:

<!-- TODO: this needs to be kept up to date -->
```rust,ignore
riot_rs::group_peripherals!(Peripherals {
    leds: LedPeripherals,
    buttons: ButtonPeripherals,
});
```

Similarly to `LedPeripherals`, an instance of the `Peripherals` RIOT-rs peripheral struct thus defined can be obtained with [`spawner` or `task`][spawner-or-task].

## The `spawner` and `task` RIOT-rs macros

Unlike traditional Rust programs, RIOT-rs applications do not have a single entrypoint.
Instead, multiple functions can be registered to execute during boot.
Functions can currently be registered as either `spawner`s or `task`s:

<!-- TODO: technically the Spawner links are for Cortex-M only -->
- [`spawner` functions][spawner-attr-docs] are non-`async` and should be used when no `async` functions need to be called.
  They are provided with a [`Spawner`](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.Spawner.html) instance and can therefore be used to [`spawn`](https://docs.embassy.dev/embassy-executor/git/cortex-m/struct.Spawner.html#method.spawn) other `async` tasks.
- [`task` functions][task-attr-docs] are `async` functions that are statically allocated at compile-time.
  They are especially useful for long-running, `async` tasks.
  They are also required to use *RIOT-rs configuration hooks*, which can be requested with their associated macro parameter, and allow to provide configuration during boot.
  Please refer to the documentation of [`task`][task-attr-docs] for a list of available hooks and to [Configuration Hooks](#configuration-hooks) to know more about hook usage.

Both of these can be provided with an instance of a RIOT-rs peripheral struct when needed, using the `peripherals` macro parameters (see the macros' documentation) and taking that RIOT-rs peripheral struct as parameter.

> The Embassy peripherals obtained this way are regular peripherals from Embassy, which are compatible with both RIOT-rs portable drivers and [Embassy architecture crates'][embassy-architecture-crates] architecture-specific drivers.

### Examples

Here is an example of the `task` macro (the `pins` module internally uses `define_peripherals!`) from the [`blinky` example][blinky-example-src]:

```rust,ignore
{{ #include ../../examples/blinky/src/main.rs:task-example-0 }}
{{ #include ../../examples/blinky/src/main.rs:task-example-1 }}
```

## Configuration Hooks

TODO

## Threading

TODO

[embassy-architecture-crates]: ./glossary.md#embassy-architecture-crates
[spawner-attr-docs]: https://future-proof-iot.github.io/RIOT-rs/dev/docs/api/riot_rs/attr.spawner.html
[task-attr-docs]: https://future-proof-iot.github.io/RIOT-rs/dev/docs/api/riot_rs/attr.task.html
[spawner-or-task]: #the-spawner-and-task-riot-rs-macros
[blinky-example-src]: https://github.com/future-proof-iot/RIOT-rs/tree/main/examples/blinky
[define_peripherals-docs]: https://future-proof-iot.github.io/RIOT-rs/dev/docs/api/riot_rs/macro.define_peripherals.html
[group_peripherals-docs]: https://future-proof-iot.github.io/RIOT-rs/dev/docs/api/riot_rs/macro.group_peripherals.html
