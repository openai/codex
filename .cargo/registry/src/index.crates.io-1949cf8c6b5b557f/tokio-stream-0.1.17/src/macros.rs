macro_rules! cfg_fs {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "fs")]
            #[cfg_attr(docsrs, doc(cfg(feature = "fs")))]
            $item
        )*
    }
}

macro_rules! cfg_io_util {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "io-util")]
            #[cfg_attr(docsrs, doc(cfg(feature = "io-util")))]
            $item
        )*
    }
}

macro_rules! cfg_net {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "net")]
            #[cfg_attr(docsrs, doc(cfg(feature = "net")))]
            $item
        )*
    }
}

macro_rules! cfg_time {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "time")]
            #[cfg_attr(docsrs, doc(cfg(feature = "time")))]
            $item
        )*
    }
}

macro_rules! cfg_sync {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "sync")]
            #[cfg_attr(docsrs, doc(cfg(feature = "sync")))]
            $item
        )*
    }
}

macro_rules! cfg_signal {
    ($($item:item)*) => {
        $(
            #[cfg(feature = "signal")]
            #[cfg_attr(docsrs, doc(cfg(feature = "signal")))]
            $item
        )*
    }
}
