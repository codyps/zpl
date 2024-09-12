pub struct PrinterDefinition {
    /// dots per inch of this printer.
    ///
    /// On physical printers, this is fixed in hardware.
    pub dpi: u32,

    /// Width of the label in dots.
    ///
    /// `^PW` changes the effective width, with this forming the maximum and default value.
    ///
    /// On real printers, ???
    pub width_in_dots: u32,

    /// Length of the label in dots.
    ///
    /// On real printers, this is detected durring calibration.
    pub label_length_in_dots: u32,
}

/// Information about the state of formatting tracked while formatting labels from ZPL.
pub struct FormatContext {}

pub struct ActionIter<'a> {
    context: FormatContext,
    input: &'a [crate::command::Command],
}

/// lower level drawing actions forming paths and text layout.
pub enum Action {
    //
}

impl Iterator for ActionIter<'_> {
    type Item = Action;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}
