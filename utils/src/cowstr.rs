use std::fmt;
use std::ops::Deref;

#[derive(Debug, Clone)]
pub enum CowStr<'a> {
    Ref(&'a str),
    Owned(Box<str>),
}

impl fmt::Display for CowStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Ref(s) => write!(f, "{}", s),
            Self::Owned(ref s) => write!(f, "{}", s),
        }
    }
}

impl<'a> From<&'a str> for CowStr<'a> {
    fn from(string: &'a str) -> Self {
        Self::Ref(string)
    }
}

impl From<Box<str>> for CowStr<'_> {
    fn from(string: Box<str>) -> Self {
        Self::Owned(string)
    }
}

impl From<String> for CowStr<'_> {
    fn from(string: String) -> Self {
        Self::Owned(string.into_boxed_str())
    }
}

impl CowStr<'_> {
    pub fn own_or_copy(self) -> String {
        match self {
            Self::Ref(s) => s.to_string(),
            Self::Owned(s) => String::from(s),
        }
    }
}

impl<'a> Deref for CowStr<'a> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Ref(s) => s,
            Self::Owned(ref s) => &s,
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn deref_owned() {
        const DATA: &'static str = "hello";

        let my_string = String::from(DATA);
        let cow = CowStr::from(my_string);

        assert_eq!(&cow, DATA);
    }
}
