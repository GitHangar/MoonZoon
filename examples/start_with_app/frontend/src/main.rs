use zoon::*;

#[static_ref]
fn counter() -> &'static Mutable<i32> {
    Mutable::new(0)
}

fn increment() {
    counter().update(|counter| counter + 1)
}

fn decrement() {
    counter().update(|counter| counter - 1)
}

fn root() -> impl IntoElementIterator {
    element_vec![
        Button::new().label("-").on_press(decrement),
        Text::with_signal(counter().signal()),
        Button::new().label("+").on_press(increment),
    ]
}

fn main() {
    start_app("app", root);
}
