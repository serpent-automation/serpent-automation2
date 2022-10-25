use silkenweb::{elements::html::DivBuilder, node::element::ElementBuilder};
use silkenweb_bootstrap::utility::{Colour, SetBorder, SetSpacing, Shadow, Side, Size::Size3};

use crate::css;

pub trait SpeechBubble {
    fn speech_bubble(self) -> Self;
}

impl SpeechBubble for DivBuilder {
    fn speech_bubble(self) -> Self {
        self.class(css::SPEECH_BUBBLE_BELOW)
            .margin_on_side((Some(Size3), Side::Top))
            .margin_on_side((Some(Size3), Side::End))
            .padding(Size3)
            .border(true)
            .border_colour(Colour::Secondary)
            .rounded_border(true)
            .shadow(Shadow::Medium)
    }
}
