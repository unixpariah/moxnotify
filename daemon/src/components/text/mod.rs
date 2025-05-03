pub mod body;
pub mod summary;

use super::Component;
use glyphon::FontSystem;

pub trait Text: Component {
    fn set_text<T>(&mut self, font_system: &mut FontSystem, text: T)
    where
        T: AsRef<str>;
}
