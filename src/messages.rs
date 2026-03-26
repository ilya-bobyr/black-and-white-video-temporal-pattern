use std::path::PathBuf;

/// Definition of different messages that processing threads exchange with each other.

/// Dispatcher → decoder threads.
///
/// Sequential `grid_size` frames are combined into one group.  They will be processed as one
/// texture.
pub(crate) struct DecodeWork {
    /// `grid_size` paths that form input frames in this group.  They are in order.
    pub(crate) input_paths: Vec<PathBuf>,
    /// `grid_size` paths that form output frames in this group.  They are in order.
    pub(crate) output_paths: Vec<PathBuf>,
}

// Decoder threads → GPU thread.
//
// This is a group of `grid_size` frames, that we read from files and decoded.  Now it needs to be
// transformed, according to our dithering algorithm.
pub(crate) struct TransformWork {
    /// Width of the images in `input_images`.  They will have have the same size.
    pub(crate) width: u32,
    /// Height of the images in `input_images`.  They will have have the same size.
    pub(crate) height: u32,
    /// Data for `grid_size` images, encoded as RGBA 8 bit per channel.  They are in order.
    pub(crate) input_images: Vec<Vec<u8>>,
    /// `grid_size` paths that form output frames in this group.  They are in order.
    pub(crate) output_paths: Vec<PathBuf>,
}

pub(crate) struct EncodeWork {
    /// Width of the images in `output_images`.  They will have have the same size.
    pub(crate) width: u32,
    /// Height of the images in `output_images`.  They will have have the same size.
    pub(crate) height: u32,
    /// `grid_size` images, encoded as an 8 bit greyscale value, that form output frames in this
    /// group.  They are in order.
    pub(crate) output_images: Vec<Vec<u8>>,
    /// `grid_size` paths that form output frames in this group.  They are in order.
    pub(crate) output_paths: Vec<PathBuf>,
}
