use crate::*;
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::Arc;

// ------ ------
//    Element
// ------ ------

pub struct Text {
    raw_text: RawText,
}

impl ElementUnchecked for Text {
    fn into_raw_unchecked(self) -> RawElOrText {
        self.raw_text.into()
    }
}

impl Element for Text {}

impl Text {
    #[track_caller]
    pub fn new<'a>(text: impl IntoCowStr<'a>) -> Self {
        Self {
            raw_text: RawText::new(text.into_cow_str()),
        }
    }

    #[track_caller]
    pub fn with_signal<'a>(
        text: impl Signal<Item = impl IntoCowStr<'a>> + Unpin + 'static,
    ) -> Self {
        Self {
            raw_text: RawText::with_signal(text),
        }
    }
}

// ------ ------
//  IntoElement
// ------ ------

impl<'a> IntoElement<'a> for String {
    type EL = Text;
    fn into_element(self) -> Self::EL {
        Text::new(self)
    }
}

impl<'a> IntoElement<'a> for &String {
    type EL = Text;
    fn into_element(self) -> Self::EL {
        Text::new(self)
    }
}

impl<'a> IntoElement<'a> for &str {
    type EL = Text;
    fn into_element(self) -> Self::EL {
        Text::new(self)
    }
}

impl<'a> IntoElement<'a> for Cow<'_, str> {
    type EL = Text;
    fn into_element(self) -> Self::EL {
        Text::new(self)
    }
}

impl<'a, T: IntoCowStr<'a> + Clone> IntoElement<'a> for Arc<T> {
    type EL = Text;
    fn into_element(self) -> Self::EL {
        // @TODO refactor the expression below once `Arc::unwrap_or_clone` is stable
        Text::new(Arc::try_unwrap(self).unwrap_or_else(|arc| (*arc).clone()))
    }
}

impl<'a, T: IntoCowStr<'a> + Clone> IntoElement<'a> for Rc<T> {
    type EL = Text;
    fn into_element(self) -> Self::EL {
        // @TODO refactor the expression below once `Rc::unwrap_or_clone` is stable
        Text::new(Rc::try_unwrap(self).unwrap_or_else(|rc| (*rc).clone()))
    }
}

macro_rules! make_into_element_impls {
    ($($type:ty),*) => (
        $(
        impl<'a> IntoElement<'a> for $type {
            type EL = Text;
            fn into_element(self) -> Self::EL {
                Text::new(self)
            }
        }
        )*
    )
}
make_into_element_impls!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);
