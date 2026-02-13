use thiserror::Error;

use crate::util::Dimension;

#[derive(Error, Debug)]
pub enum FontFileError {
    #[error("Failed to open font file: {source}")]
    FailedToOpen {
        #[from]
        source: std::io::Error,
    },

    #[error("Failed to decode font file: {source}")]
    FailedToDecode {
        #[from]
        source: image::ImageError,
    },

    #[error("Invalid font file dimensions {dimensions}")]
    InvalidFontFileDimensions { dimensions: Dimension<u32> },

    #[error("Invalid font file width {width}")]
    InvalidFontFileWidth { width: u32 },

    #[error("Invalid font file height {height}")]
    InvalidFontFileHeight { height: u32 },
}
