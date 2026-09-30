use std::cell::Cell;

/// How deep elements and objects nest before the parser reads the rest as
/// plain text. It bounds the parser's stack use, and the depth of the tree
/// every consumer walks.
pub const MAX_NESTING: u32 = 64;

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// One nesting level, held while its contents are parsed.
pub struct Nesting(());

impl Nesting {
    pub fn enter() -> Nesting {
        DEPTH.with(|depth| depth.set(depth.get() + 1));
        Nesting(())
    }
}

impl Drop for Nesting {
    fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(depth.get() - 1));
    }
}

pub fn too_deep() -> bool {
    DEPTH.with(|depth| depth.get() > MAX_NESTING)
}
