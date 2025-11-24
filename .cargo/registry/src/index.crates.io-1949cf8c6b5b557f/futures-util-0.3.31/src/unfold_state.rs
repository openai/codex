use core::pin::Pin;

use pin_project_lite::pin_project;

pin_project! {
    /// UnfoldState used for stream and sink unfolds
    #[project = UnfoldStateProj]
    #[project_replace = UnfoldStateProjReplace]
    #[derive(Debug)]
    pub(crate) enum UnfoldState<T, R> {
        Value {
            value: T,
        },
        Future {
            #[pin]
            future: R,
        },
        Empty,
    }
}

impl<T, R> UnfoldState<T, R> {
    pub(crate) fn project_future(self: Pin<&mut Self>) -> Option<Pin<&mut R>> {
        match self.project() {
            UnfoldStateProj::Future { future } => Some(future),
            _ => None,
        }
    }

    pub(crate) fn take_value(self: Pin<&mut Self>) -> Option<T> {
        match &*self {
            Self::Value { .. } => match self.project_replace(Self::Empty) {
                UnfoldStateProjReplace::Value { value } => Some(value),
                _ => unreachable!(),
            },
            _ => None,
        }
    }
}
