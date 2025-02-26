use crate::{style::supports_dvx, *};
use std::collections::BTreeMap;
use std::rc::Rc;
use strum::{EnumIter, IntoEnumIterator, IntoStaticStr};

/// Styling for height.
#[derive(Default, Clone)]
pub struct Height<'a> {
    css_props: BTreeMap<CssName, Option<Rc<CssPropValue<'a>>>>,
    height_mode: HeightMode,
    self_signal: Option<Broadcaster<LocalBoxSignal<'static, Option<Self>>>>,
}

fn into_prop_value<'a>(value: impl IntoCowStr<'a>) -> Option<Rc<CssPropValue<'a>>> {
    Some(Rc::new(CssPropValue::new(value)))
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumIter, IntoStaticStr)]
#[strum(serialize_all = "kebab-case")]
enum CssName {
    MinHeight,
    Height,
    MaxHeight,
    FlexGrow,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, EnumIter, IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
enum HeightMode {
    #[default]
    ExactHeight,
    FillHeight,
}

impl<'a> Height<'a> {
    /// Define the height with pixels for an element.
    /// # Example
    /// ```no_run
    /// use zoon::*;
    ///
    /// let button = Button::new().s(Height::exact(50)).label("Click me");
    /// ```
    pub fn exact(height: u32) -> Self {
        let mut this = Self::default();
        this.css_props
            .insert(CssName::Height, into_prop_value(px(height)));
        this.height_mode = HeightMode::ExactHeight;
        this
    }

    /// Define the height with pixels depending of signal's state.
    /// # Example
    /// ```no_run
    /// use zoon::*;
    ///
    /// let (is_hovered, hover_signal) = Mutable::new_and_signal(false);
    /// let button = Button::new()
    ///     .s(Height::exact_signal(hover_signal.map_bool(|| 50, || 100)))
    ///     .on_hovered_change(move |hover| is_hovered.set(hover))
    ///     .label("hover me");
    /// ```
    pub fn exact_signal(
        height: impl Signal<Item = impl Into<Option<u32>>> + Unpin + 'static,
    ) -> Self {
        Self::with_signal(height.map(|height| height.into().map(Height::exact)))
    }

    pub fn with_signal(
        height: impl Signal<Item = impl Into<Option<Self>>> + Unpin + 'static,
    ) -> Self {
        let mut this = Self::default();
        let height = height.map(|height| height.into());
        this.self_signal = Some(height.boxed_local().broadcast());
        this
    }

    /// Set the element height to fill its container.
    /// # Example
    /// ```no_run
    /// use zoon::*;
    ///
    /// let button = Button::new()
    ///     .s(Height::fill())
    ///     .label("Hover this giant button");
    /// ```
    pub fn fill() -> Self {
        let mut this = Self::default();
        this.css_props
            .insert(CssName::Height, into_prop_value("100%"));
        this.height_mode = HeightMode::FillHeight;
        this
    }

    pub fn percent(percent: impl Into<f64>) -> Self {
        let mut this = Self::default();
        this.css_props
            .insert(CssName::Height, into_prop_value(pct(percent.into())));
        this.height_mode = HeightMode::ExactHeight;
        this
    }

    pub fn percent_signal<T: Into<f64>>(
        height: impl Signal<Item = impl Into<Option<T>>> + Unpin + 'static,
    ) -> Self {
        Self::with_signal(
            height.map(|height| height.into().map(|height| Height::percent(height.into()))),
        )
    }

    pub fn growable() -> Self {
        Self::growable_with_factor::<f64>(None)
    }

    pub fn growable_with_factor<T: Into<f64>>(factor: impl Into<Option<T>>) -> Self {
        let mut this = Self::default();
        this.css_props
            .insert(CssName::Height, into_prop_value("auto"));
        if let Some(factor) = factor.into() {
            this.css_props
                .insert(CssName::FlexGrow, into_prop_value(factor.into()));
        }
        this.height_mode = HeightMode::FillHeight;
        this
    }

    /// The element height will be the height of the device screen or web
    /// browser frame.
    /// # Example
    /// ```no_run
    /// use zoon::*;
    ///
    /// let button = Button::new()
    ///     .s(Height::screen())
    ///     .label("Hover this giant button");
    /// ```
    pub fn screen() -> Self {
        let mut this = Self::default();
        this.css_props.insert(
            CssName::Height,
            into_prop_value(if *supports_dvx() { "100dvh" } else { "100vh" }),
        );
        this.height_mode = HeightMode::ExactHeight;
        this
    }

    pub fn min(mut self, height: u32) -> Self {
        self.css_props
            .insert(CssName::MinHeight, into_prop_value(px(height)));
        self
    }

    /// The element minimum height will be the height of thw device screen or
    /// web browser frame.
    /// # Example
    /// ```no_run
    /// use zoon::*;
    ///
    /// let button = Button::new()
    ///     .s(Height::fill().min_screen())
    ///     .label("Hover this giant button");
    /// ```
    pub fn min_screen(mut self) -> Self {
        self.css_props.insert(
            CssName::MinHeight,
            into_prop_value(if *supports_dvx() { "100dvh" } else { "100vh" }),
        );
        self
    }

    /// The element maximum height can be set by value in pixels.
    /// # Example
    /// ```no_run
    /// use zoon::*;
    ///
    /// let button = Button::new()
    ///     .s(Height::fill().max(150))
    ///     .label("Hover this giant button");
    /// ```
    pub fn max(mut self, height: u32) -> Self {
        self.css_props
            .insert(CssName::MaxHeight, into_prop_value(px(height)));
        self
    }

    /// Set the maximum element height to fill its container.
    /// # Example
    /// ```no_run
    /// use zoon::*;
    ///
    /// let button = Button::new()
    ///     .s(Height::fill().max_fill())
    ///     .label("Hover this giant button");
    /// ```
    pub fn max_fill(mut self) -> Self {
        self.css_props
            .insert(CssName::MaxHeight, into_prop_value("100%"));
        self
    }
}

impl<'a> Style<'a> for Height<'static> {
    fn move_to_groups(self, groups: &mut StyleGroups<'a>) {
        groups.update_first(|mut group| {
            let Self {
                css_props,
                height_mode,
                self_signal,
            } = self;

            if let Some(self_signal) = self_signal {
                for name in CssName::iter() {
                    group = group.style_signal(
                        <&str>::from(name),
                        self_signal.signal_ref(move |this| {
                            this.as_ref().and_then(|this| {
                                this.css_props.get(&name).and_then(|value| {
                                    value.clone().map(|value| {
                                        // @TODO refactor the expression below once `Rc::unwrap_or_clone` is stable
                                        Rc::try_unwrap(value)
                                            .unwrap_or_else(|rc| (*rc).clone())
                                            .value
                                    })
                                })
                            })
                        }),
                    );
                }

                for mode in HeightMode::iter() {
                    group = group.class_signal(
                        <&str>::from(mode),
                        self_signal.signal_ref(move |this| {
                            this.as_ref()
                                .map(|this| this.height_mode == mode)
                                .unwrap_or_default()
                        }),
                    );
                }
                group
            } else {
                group
                    .static_css_props
                    .extend(css_props.into_iter().map(|(name, mut value)| {
                        (
                            name.into(),
                            // @TODO refactor the expression below once `Rc::unwrap_or_clone` is stable
                            Rc::try_unwrap(value.take().unwrap_throw())
                                .unwrap_or_else(|rc| (*rc).clone()),
                        )
                    }));
                group.class(height_mode.into())
            }
        });
    }
}
