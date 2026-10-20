//! Engine of the names example: keeps what the commands send.

use bern::CommandError;
use names_protocol::names::{
    self, Align, Alignment, Animation, Arrangement, Bundle, Button, Color, Content, Data, Date,
    Error, Event, Font, Image, Layout, List, Locale, Map, Modifier, Notification, Orientation,
    Pair, Path, Role, Scene, Size, State, Task, Text, Timer, Toggle, View, Widget,
};

pub struct NamesEngine {
    state: State,
}

impl Default for NamesEngine {
    fn default() -> Self {
        Self {
            state: State {
                layout: Layout::Column,
                color: Color::Red,
                view: View::Grid,
                alignment: Alignment::Leading,
                modifier: Modifier::Bold,
                error: None,
                font: Font::Body,
                animation: Animation::Spring,
                task: Task::Pending,
                button: Button::Primary,
                toggle: Toggle::On,
                scene: Scene::Day,
                locale: Locale::Here,
                timer: Timer::Stopped,
                bundle: Bundle::Small,
                role: Role::Owner,
                arrangement: Arrangement::Spaced,
                orientation: Orientation::Horizontal,
                align: Align::Start,
                widget: Widget::Small,
                event: Event::Tap,
                map: Map::Street,
                pair: Pair::One,
                notification: Notification::Banner,
                text: Text {
                    id: 0,
                    content: "Hello".into(),
                    size: 17.0,
                },
                image: Image {
                    name: "mark".into(),
                    width: 64,
                },
                list: List {
                    texts: vec![
                        Text {
                            id: 1,
                            content: "One".into(),
                            size: 13.0,
                        },
                        Text {
                            id: 2,
                            content: "Two".into(),
                            size: 13.0,
                        },
                    ],
                },
                data: Data { length: 0 },
                date: Date { day: 1 },
                path: Path { length: 0.0 },
                size: Size {
                    width: 0.0,
                    height: 0.0,
                },
                content: Content::Empty,
            },
        }
    }
}

impl names::Commands for NamesEngine {
    fn set_layout(&mut self, layout: Layout) -> Result<(), CommandError> {
        self.state.layout = layout;
        Ok(())
    }

    fn paint(&mut self, color: Color) -> Result<(), CommandError> {
        self.state.color = color;
        Ok(())
    }

    fn show(&mut self, content: Content) -> Result<(), CommandError> {
        self.state.content = content;
        Ok(())
    }

    fn add_text(&mut self, text: Text) -> Result<(), CommandError> {
        self.state.list.texts.push(text);
        Ok(())
    }

    fn fail(&mut self, error: Option<Error>) -> Result<(), CommandError> {
        self.state.error = error;
        Ok(())
    }
}

impl names::Engine for NamesEngine {
    fn state(&self) -> &State {
        &self.state
    }
}
