use iced::{
    advanced::{
        layout::{self, Layout},
        renderer,
        widget::{self, Widget},
    },
    mouse::Cursor,
};
use iced::{Color, Element, Length, Rectangle, Size};

pub struct Circle {
    radius: f32,
    color: Color,
}

impl Circle {
    pub fn new(radius: f32, color: Color) -> Self {
        Self { radius, color }
    }
}

pub fn circle(radius: f32, color: Color) -> Circle {
    Circle::new(radius, color)
}

impl<Message, Renderer> Widget<Message, Renderer> for Circle
where
    Renderer: iced::advanced::Renderer,
{
    fn width(&self) -> Length {
        Length::Shrink
    }

    fn height(&self) -> Length {
        Length::Shrink
    }

    fn layout(&self, _renderer: &Renderer, _limits: &layout::Limits) -> layout::Node {
        layout::Node::new(Size::new(self.radius * 2.0, self.radius * 2.0))
    }

    fn draw(
        &self,
        _state: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Renderer::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        renderer.fill_quad(
            renderer::Quad {
                bounds: layout.bounds(),
                border_radius: self.radius.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
            },
            self.color,
        );
    }
}

impl<'a, Message, Renderer> From<Circle> for Element<'a, Message, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn from(circle: Circle) -> Self {
        Self::new(circle)
    }
}
