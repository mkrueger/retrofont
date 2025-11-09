use crate::{FontError, FontTarget, FontType, RenderMode, Result};

pub trait Font: Send + Sync {
    fn name(&self) -> &str;
    fn font_type(&self) -> FontType;
    fn has_char(&self, ch: char) -> bool;
    fn render_char<T: FontTarget>(&self, target: &mut T, ch: char, mode: RenderMode) -> Result<()>;

    fn render_str<T: FontTarget>(
        &self,
        target: &mut T,
        text: &str,
        mode: RenderMode,
    ) -> Result<()> {
        for ch in text.chars() {
            self.render_char(target, ch, mode)?;
            target.next_line().map_err(|_| FontError::InvalidGlyph)?;
        }
        Ok(())
    }
}
