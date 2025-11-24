// Full credit to @tesaguri who posted this gist under CC0 1.0 Universal licence
// https://gist.github.com/tesaguri/2a1c0790a48bbda3dd7f71c26d02a793

macro_rules! pin_project_cfg {
    ($(#[$($attr:tt)*])* $vis:vis enum $($rest:tt)+) => {
        pin_project_cfg! {
            @outer [$(#[$($attr)*])* $vis enum] $($rest)+
        }
    };
    // Accumulate type parameters and `where` clause.
    (@outer [$($accum:tt)*] $tt:tt $($rest:tt)+) => {
        pin_project_cfg! {
            @outer [$($accum)* $tt] $($rest)+
        }
    };
    (@outer [$($accum:tt)*] { $($body:tt)* }) => {
        pin_project_cfg! {
            @body #[cfg(all())] [$($accum)*] {} $($body)*
        }
    };
    // Process a variant with `cfg`.
    (
        @body
        #[cfg(all($($pred_accum:tt)*))]
        $outer:tt
        { $($accum:tt)* }

        #[cfg($($pred:tt)*)]
        $(#[$($attr:tt)*])* $variant:ident { $($body:tt)* },
        $($rest:tt)*
    ) => {
        // Create two versions of the enum with `cfg($pred)` and `cfg(not($pred))`.
        pin_project_cfg! {
            @variant_body
            { $($body)* }
            {}
            #[cfg(all($($pred_accum)* $($pred)*,))]
            $outer
            { $($accum)* $(#[$($attr)*])* $variant }
            $($rest)*
        }
        pin_project_cfg! {
            @body
            #[cfg(all($($pred_accum)* not($($pred)*),))]
            $outer
            { $($accum)* }
            $($rest)*
        }
    };
    // Process a variant without `cfg`.
    (
        @body
        #[cfg(all($($pred_accum:tt)*))]
        $outer:tt
        { $($accum:tt)* }

        $(#[$($attr:tt)*])* $variant:ident { $($body:tt)* },
        $($rest:tt)*
    ) => {
        pin_project_cfg! {
            @variant_body
            { $($body)* }
            {}
            #[cfg(all($($pred_accum)*))]
            $outer
            { $($accum)* $(#[$($attr)*])* $variant }
            $($rest)*
        }
    };
    // Process a variant field with `cfg`.
    (
        @variant_body
        {
            #[cfg($($pred:tt)*)]
            $(#[$($attr:tt)*])* $field:ident: $ty:ty,
            $($rest:tt)*
        }
        { $($accum:tt)* }
        #[cfg(all($($pred_accum:tt)*))]
        $($outer:tt)*
    ) => {
        pin_project_cfg! {
            @variant_body
            {$($rest)*}
            { $($accum)* $(#[$($attr)*])* $field: $ty, }
            #[cfg(all($($pred_accum)* $($pred)*,))]
            $($outer)*
        }
        pin_project_cfg! {
            @variant_body
            { $($rest)* }
            { $($accum)* }
            #[cfg(all($($pred_accum)* not($($pred)*),))]
            $($outer)*
        }
    };
    // Process a variant field without `cfg`.
    (
        @variant_body
        {
            $(#[$($attr:tt)*])* $field:ident: $ty:ty,
            $($rest:tt)*
        }
        { $($accum:tt)* }
        $($outer:tt)*
    ) => {
        pin_project_cfg! {
            @variant_body
            {$($rest)*}
            { $($accum)* $(#[$($attr)*])* $field: $ty, }
            $($outer)*
        }
    };
    (
        @variant_body
        {}
        $body:tt
        #[cfg(all($($pred_accum:tt)*))]
        $outer:tt
        { $($accum:tt)* }
        $($rest:tt)*
    ) => {
        pin_project_cfg! {
            @body
            #[cfg(all($($pred_accum)*))]
            $outer
            { $($accum)* $body, }
            $($rest)*
        }
    };
    (
        @body
        #[$cfg:meta]
        [$($outer:tt)*]
        $body:tt
    ) => {
        #[$cfg]
        pin_project_lite::pin_project! {
            $($outer)* $body
        }
    };
}

pub(crate) use pin_project_cfg;
