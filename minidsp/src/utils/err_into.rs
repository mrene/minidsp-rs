use std::result::Result;

pub trait ErrInto<T, ESource> {
    fn err_into<EDest>(self) -> Result<T, EDest>
    where
        ESource: Into<EDest>;
}

impl<T, ESource> ErrInto<T, ESource> for Result<T, ESource> {
    fn err_into<EDest>(self) -> Result<T, EDest>
    where
        ESource: Into<EDest>,
    {
        self.map_err(|e| e.into())
    }
}
