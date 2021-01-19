use std::result::Result;

pub trait ErrInto<T, ESource, EDest> {
    fn err_into(self) -> Result<T, EDest>;
}

impl<T, ESource, EDest> ErrInto<T, ESource, EDest> for Result<T, ESource>
where
    ESource: Into<EDest>,
{
    fn err_into(self) -> Result<T, EDest> {
        self.map_err(|e| e.into())
    }
}
