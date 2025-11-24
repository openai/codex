macro_rules! cfg_feature {
    (
        #![$meta:meta]
        $($item:item)*
    ) => {
        $(
            #[cfg($meta)]
            #[cfg_attr(docsrs, doc(cfg($meta)))]
            $item
        )*
    }
}

macro_rules! cfg_proto {
    ($($item:item)*) => {
        cfg_feature! {
            #![all(
                any(feature = "http1", feature = "http2"),
                any(feature = "client", feature = "server"),
            )]
            $($item)*
        }
    }
}

cfg_proto! {
    macro_rules! cfg_client {
        ($($item:item)*) => {
            cfg_feature! {
                #![feature = "client"]
                $($item)*
            }
        }
    }

    macro_rules! cfg_server {
        ($($item:item)*) => {
            cfg_feature! {
                #![feature = "server"]
                $($item)*
            }
        }
    }
}
